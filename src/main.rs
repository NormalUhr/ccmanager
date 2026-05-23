use ccmanager::cli::{Args, Commands};
use ccmanager::error::{AppError, Result};
use ccmanager::{config, debug, debug_log, display, history, launcher, tui, update};
use clap::Parser;
use std::io::IsTerminal;

fn main() {
    if let Err(e) = run() {
        match e {
            AppError::SelectionCancelled => {
                // User cancelled, exit silently
                std::process::exit(0);
            }
            _ => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Helper function to resolve a boolean setting by merging CLI flags and config values.
///
/// Priority: enable_flag > disable_flag > config_value > default_value
fn resolve_bool_setting(
    enable_flag: bool,
    disable_flag: bool,
    config_value: Option<bool>,
    default_value: bool,
) -> bool {
    if enable_flag {
        true
    } else if disable_flag {
        false
    } else {
        config_value.unwrap_or(default_value)
    }
}

fn run() -> Result<()> {
    let args = Args::parse();

    // Handle subcommands
    if let Some(command) = args.command {
        return match command {
            Commands::Update => update::run(),
            #[cfg(feature = "mcp")]
            Commands::Mcp => ccmanager::mcp::serve(),
            #[cfg(feature = "serve")]
            Commands::Serve {
                host,
                port,
                token,
                open,
                read_only,
            } => ccmanager::web::serve(ccmanager::web::ServeConfig {
                host,
                port,
                token,
                open,
                read_only,
            }),
        };
    }

    // Detect terminal theme before entering raw mode / alternate screen,
    // as terminal_light queries the terminal for background color
    tui::theme::detect_theme();

    let config = config::load_config()?;

    // Merge CLI arguments with config file settings. CLI takes precedence.
    let display_config = config.display.unwrap_or_default();

    // Extract resume config
    let resume_config = config.resume.unwrap_or_default();
    let default_args = resume_config.default_args.as_deref().unwrap_or(&[]);
    // Default to skip permissions for the primary resume binding. Users who
    // want the standard permission flow as their default can flip this in
    // their config; the alt binding (Alt+R/Alt+F) always inverts whichever
    // is configured as primary.
    let default_skip_permissions = resume_config.skip_permissions.unwrap_or(true);
    // CLI override for the non-TUI `ccmanager --resume` path.
    let cli_skip_permissions = if args.with_permissions {
        false
    } else {
        default_skip_permissions
    };

    // Disable colors globally when --no-color is passed
    if args.no_color {
        colored::control::set_override(false);
    }

    // Resolve keybindings
    let keys = config::KeyBindings::from_config(config.keys);

    // Use positive names internally for clarity
    let show_tools = resolve_bool_setting(
        args.show_tools,
        args.no_tools,
        display_config.no_tools.map(|b| !b),
        false, // Default: hide tools
    );
    // Map CLI flag to ToolDisplayMode
    // --show-tools → Full, --no-tools → Hidden, default → Hidden
    // The default is a clean dialogue-only view; press `t` in the viewer to
    // cycle through hidden → truncated → full. Override with `--show-tools`
    // or set `no_tools = false` in config to start with tools visible.
    let tool_display = if args.show_tools {
        tui::ToolDisplayMode::Full
    } else if args.no_tools {
        tui::ToolDisplayMode::Hidden
    } else {
        match display_config.no_tools {
            Some(true) => tui::ToolDisplayMode::Hidden,
            Some(false) => tui::ToolDisplayMode::Full,
            None => tui::ToolDisplayMode::Hidden,
        }
    };
    let show_last = resolve_bool_setting(args.last, args.first, display_config.last, true);
    let show_thinking = resolve_bool_setting(
        args.show_thinking,
        args.hide_thinking,
        display_config.show_thinking,
        false,
    );
    let plain_mode = resolve_bool_setting(args.plain, false, display_config.plain, false);
    let use_pager = resolve_bool_setting(
        args.pager,
        args.no_pager,
        display_config.pager,
        std::io::stdout().is_terminal(),
    );

    // Handle --delete flag: delete a session by UUID and exit
    if let Some(ref session_id) = args.delete {
        match history::delete_session_by_uuid(session_id) {
            Ok(count) => {
                if count == 1 {
                    eprintln!("Deleted session {}", session_id);
                } else {
                    eprintln!(
                        "Deleted session {} ({} copies across projects)",
                        session_id, count
                    );
                }
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }

    // Handle --debug-search flag: debug search result scoring
    if let Some(ref query) = args.debug_search {
        let mut conversations = history::load_all_conversations(show_last, args.debug)?;
        conversations.sort_by_key(|c| std::cmp::Reverse(c.timestamp));

        let searchable = tui::search::precompute_search_text(&conversations);
        let now = chrono::Local::now();

        let query_lower = tui::search::normalize_for_search(query);
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let adjacent_pairs: Vec<String> = if query_words.len() > 1 {
            query_words
                .windows(2)
                .map(|w| format!("{} {}", w[0], w[1]))
                .collect()
        } else {
            vec![]
        };

        // Optionally filter to local workspace
        let current_project_dir_name = if args.local {
            std::env::current_dir()
                .ok()
                .map(|d| history::convert_path_to_project_dir_name(&d))
        } else {
            None
        };

        let mut results: Vec<_> = searchable
            .iter()
            .filter_map(|s| {
                if let Some(ref proj) = current_project_dir_name {
                    let conv = &conversations[s.index];
                    let matches =
                        conv.path
                            .parent()
                            .and_then(|p| p.file_name())
                            .is_some_and(|name| {
                                history::is_same_project(&name.to_string_lossy(), proj)
                            });
                    if !matches {
                        return None;
                    }
                }

                let debug = tui::search::score_text_debug(
                    s,
                    &conversations[s.index].search_text_lower,
                    &query_words,
                    &adjacent_pairs,
                    conversations[s.index].timestamp,
                    now,
                )?;
                Some((s.index, debug))
            })
            .collect();

        results.sort_by(|a, b| {
            b.1.total
                .partial_cmp(&a.1.total)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (rank, (idx, debug)) in results.iter().take(30).enumerate() {
            let conv = &conversations[*idx];
            let age = now.signed_duration_since(conv.timestamp);
            let project = conv.project_name.as_deref().unwrap_or("(none)");
            let age_str = format_debug_age(age);
            let session = conv
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?");
            eprintln!(
                "#{:2} score={:.2} freshness={:.2} | {} | {} | {} ago",
                rank + 1,
                debug.total,
                debug.freshness,
                project,
                session,
                age_str
            );

            for field in &debug.fields {
                if field.tf_score > 0.0 || field.adjacency_score > 0.0 {
                    eprintln!(
                        "     {}: tf={:.2} adj={:.2} (w={:.1})",
                        field.name, field.tf_score, field.adjacency_score, field.weight
                    );
                    for (word, tf, ln_score) in &field.word_details {
                        if *tf > 0 {
                            eprintln!("       \"{}\" tf={} ln={:.2}", word, tf, ln_score);
                        }
                    }
                }
            }
            eprintln!();
        }

        return Ok(());
    }

    // Handle --render flag: render a JSONL file in ledger format and exit
    if let Some(ref render_path) = args.render {
        let display_options = display::DisplayOptions {
            no_tools: !show_tools,
            show_thinking,
            debug_level: args.debug,
            use_pager,
            no_color: args.no_color,
        };
        return display::render_to_terminal(render_path, &display_options);
    }

    // Handle direct file input mode
    if let Some(ref input_file) = args.input_file {
        if !input_file.exists() {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", input_file.display()),
            )));
        }
        if !input_file.is_file() {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Not a file: {}", input_file.display()),
            )));
        }
        tui::run_single_file(
            input_file.clone(),
            tool_display,
            show_thinking,
            keys,
            default_skip_permissions,
        )?;
        return Ok(());
    }

    let use_local = args.local;

    // Determine the current workspace's project directory name (for workspace filter)
    let current_dir = std::env::current_dir().ok();
    let current_project_dir_name = current_dir
        .as_ref()
        .map(|d| history::convert_path_to_project_dir_name(d));

    // Handle --show-dir flag (needs current_dir)
    if args.show_dir {
        if let Some(ref dir) = current_dir {
            let projects_dir = history::get_claude_projects_dir(dir)?;
            println!("{}", projects_dir.display());
            return Ok(());
        } else {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Failed to get current directory",
            )));
        }
    }

    // --local starts with workspace filter on; default is global (filter off)
    let workspace_filter = use_local;

    // Always use streaming global loader for all conversations
    let rx = history::load_all_conversations_streaming(show_last, args.debug);

    let (conversations, selected_path) = match tui::run_with_loader(
        rx,
        tool_display,
        show_thinking,
        keys,
        workspace_filter,
        current_project_dir_name,
        default_skip_permissions,
        default_args.to_vec(),
        show_last,
        args.debug,
    )? {
        (tui::Action::Select(path), convs) => (convs, path),
        (tui::Action::Quit, _) => return Err(AppError::SelectionCancelled),
        (tui::Action::Delete(_), _) => unreachable!("Delete is handled internally"),
        (tui::Action::Resume { .. }, _)
        | (tui::Action::ForkResume { .. }, _)
        | (tui::Action::Refresh, _) => {
            // Resume / ForkResume / Refresh are all handled inside the
            // TUI loop and never bubble out of `run_with_loader`.
            unreachable!("Resume / ForkResume / Refresh are handled inside the TUI loop");
        }
    };

    if args.show_path {
        println!("{}", selected_path.display());
        return Ok(());
    }

    if args.show_id {
        let conversation_id = selected_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                AppError::ClaudeExecutionError(
                    "Conversation filename is not valid Unicode".to_string(),
                )
            })?;
        println!("{}", conversation_id);
        return Ok(());
    }

    if args.resume {
        // Find the selected conversation to get its project_path
        let conv = conversations.iter().find(|c| c.path == selected_path);
        debug::debug(
            args.debug,
            &format!("Selected path: {}", selected_path.display()),
        );
        debug::debug(
            args.debug,
            &format!("Found conversation: {}", conv.is_some()),
        );
        if let Some(c) = conv {
            debug::debug(args.debug, &format!("project_path: {:?}", c.project_path));
            if let Some(p) = &c.project_path {
                debug::debug(args.debug, &format!("project_path exists: {}", p.exists()));
            }
        }
        let project_path = conv.and_then(|c| c.project_path.as_ref());
        let cwd = std::env::current_dir().map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to get current directory: {}", e),
            ))
        })?;
        let plan = launcher::plan_resume(
            &selected_path,
            project_path,
            default_args,
            args.fork_session,
            &cwd,
            cli_skip_permissions,
        )?;
        launcher::run_copy_step(&selected_path, &plan)?;
        launcher::exec_in_place(&plan)?;
        return Ok(());
    }

    // Log parse errors to debug log if debug mode is enabled
    if args.debug.is_some()
        && let Some(conv) = conversations.iter().find(|c| c.path == selected_path)
    {
        if let Err(e) = debug_log::log_parse_errors(conv) {
            debug::warn(
                args.debug,
                &format!("Failed to write parse errors to log: {}", e),
            );
        } else if !conv.parse_errors.is_empty() {
            debug::info(
                args.debug,
                &format!(
                    "Logged {} parse error(s) to ~/.local/state/ccmanager/debug.log",
                    conv.parse_errors.len()
                ),
            );
        }
    }

    // Display the selected conversation
    let display_options = display::DisplayOptions {
        no_tools: !show_tools,
        show_thinking,
        debug_level: args.debug,
        use_pager,
        no_color: args.no_color,
    };

    if plain_mode {
        display::display_conversation_plain(&selected_path, &display_options)?;
    } else {
        display::display_conversation(&selected_path, &display_options)?;
    }

    Ok(())
}

fn format_debug_age(age: chrono::Duration) -> String {
    let hours = age.num_hours();
    if hours < 24 {
        format!("{}h", hours)
    } else {
        format!("{}d", hours / 24)
    }
}
