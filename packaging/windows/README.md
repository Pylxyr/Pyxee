# installer.nsi

A copy of Tauri's NSIS template (`tauri-cli-v2.12.3`, `crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`) with
one added line: `MUI_FINISHPAGE_SHOWREADME_NOTCHECKED`, so the finish page's "Create desktop shortcut" box starts
unticked. Tauri has no setting for that. Silent and passive installs still create the shortcut, as in the original.

When `@tauri-apps/cli` is upgraded, take the new template from the same tag, add the same line again, and compare with
`diff`: the only difference must be that one line and its comment.
