# Accepted common design foundation

Charcoal surfaces, muted category colors, bright numeric values, dimmer secondary text, teal selection, rounded category framing, and proportional nested area encoding are shared across states. Terminal monospace is fixed by the capture environment; large allocated amounts use the same frozen pixel-glyph metric routine. Size/percent values carry weight; underlying byte semantics remain unchanged. Layout and action flow are judged separately.

The complete shared implementation is `src/theme.rs` and `src/ui/foundation.rs`. Its accepted combined fingerprint is `8254c4e33773bfbe859359cef0743c204d2d52a371146af2355dd631565102ad`. These sources are frozen. First-round preference was investigated with an alternate saved reference theme, not counted automatically: the same candidate was preferred over both default and installed-gruvbox btop in fresh Astra sessions. Only the robustness vote counts. Both critics identified truncation weaknesses; neither claimed the domain or feature set made the candidate win.

Final user-requested Fable rejudgment supersedes earlier Astra acceptance counts. Packet p6ade2a9d9f045c5c won on unchanged final releaseeebe549a against mature btop after independent fairness audit. The whole design round stopped with4/6product pieces won; this shared-foundation win does not imply a dense-overview or whole-interface win.
