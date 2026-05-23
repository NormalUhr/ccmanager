// ccmanager web UI — progressive enhancement.
// Keyboard shortcuts. Copy-to-clipboard helpers. No framework.

(function () {
    "use strict";

    // ------ Navigation helpers ------
    // Return to the list, preferring history.back() when that would restore
    // a prior search state (we came from /conversations?q=…). Falls through
    // to an explicit navigation for deep-linked conversation URLs with no
    // relevant history.
    function navigateBackToList() {
        try {
            const ref = document.referrer;
            if (ref) {
                const refUrl = new URL(ref, location.href);
                if (
                    refUrl.origin === location.origin &&
                    refUrl.pathname === "/conversations"
                ) {
                    history.back();
                    return;
                }
            }
        } catch (_) {
            /* fall through */
        }
        location.href = "/conversations";
    }
    // Expose for inline onclick handlers on the ← list button.
    window.navigateBackToList = navigateBackToList;

    function isOnConversationViewer() {
        return /^\/conversations\/[^/]+$/.test(location.pathname);
    }

    // ------ Keyboard shortcuts (global) ------
    // j/k: scroll; J/K: previous/next message; g/G: top/bottom; /: focus search.
    function isTypingTarget(el) {
        if (!el) return false;
        const tag = el.tagName;
        return (
            tag === "INPUT" ||
            tag === "TEXTAREA" ||
            el.isContentEditable === true
        );
    }

    function scrollByLines(dy) {
        window.scrollBy({ top: dy, behavior: "instant" });
    }

    function jumpMessage(dir) {
        const msgs = Array.from(document.querySelectorAll("[data-msg-idx]"));
        if (msgs.length === 0) return;
        const scrollTop = window.scrollY;
        let targetIdx = -1;
        for (let i = 0; i < msgs.length; i++) {
            const top = msgs[i].getBoundingClientRect().top + scrollTop;
            if (top > scrollTop + 4) {
                targetIdx = dir > 0 ? i : Math.max(0, i - 1);
                break;
            }
        }
        if (targetIdx < 0) targetIdx = dir > 0 ? msgs.length - 1 : 0;
        const el = msgs[targetIdx];
        el.scrollIntoView({ behavior: "smooth", block: "start" });
        // Briefly highlight which message is now focused.
        document
            .querySelectorAll(".message.focus")
            .forEach((n) => n.classList.remove("focus"));
        el.classList.add("focus");
    }

    document.addEventListener("keydown", function (e) {
        // Cmd+[ (macOS) / Ctrl+[ (Windows/Linux): go back to the list from
        // a conversation view. Browsers handle this natively when history
        // exists; we fire our helper as a safety net so deep-linked URLs
        // still land on /conversations instead of doing nothing.
        if (
            (e.metaKey || e.ctrlKey) &&
            !e.altKey &&
            !e.shiftKey &&
            e.key === "["
        ) {
            if (isOnConversationViewer()) {
                e.preventDefault();
                navigateBackToList();
            }
            return;
        }

        if (isTypingTarget(e.target)) return;
        if (e.metaKey || e.ctrlKey || e.altKey) return;

        switch (e.key) {
            case "j":
                scrollByLines(40);
                e.preventDefault();
                break;
            case "k":
                scrollByLines(-40);
                e.preventDefault();
                break;
            case "J":
                jumpMessage(+1);
                e.preventDefault();
                break;
            case "K":
                jumpMessage(-1);
                e.preventDefault();
                break;
            case "g":
                window.scrollTo({ top: 0, behavior: "instant" });
                e.preventDefault();
                break;
            case "G":
                window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
                e.preventDefault();
                break;
            case "/": {
                const search = document.querySelector(".search-input");
                if (search) {
                    search.focus();
                    search.select();
                    e.preventDefault();
                }
                break;
            }
        }
    });

    // ------ Copy-to-clipboard for [data-copy="..."] buttons ------
    document.addEventListener("click", function (e) {
        const btn = e.target.closest("[data-copy]");
        if (!btn) return;
        const text = btn.getAttribute("data-copy");
        if (!text) return;
        navigator.clipboard
            .writeText(text)
            .then(() => {
                const prev = btn.textContent;
                btn.textContent = "copied ✓";
                setTimeout(() => (btn.textContent = prev), 1500);
            })
            .catch(() => {
                alert("Failed to copy. Text:\n\n" + text);
            });
    });
})();
