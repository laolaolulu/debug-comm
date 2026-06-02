---
name: debug-comm-release
description: 在 debug-comm 仓库执行发布流程。用户输入“发布 v0.1.0-alpha.18 新增JS自动化脚本步骤”或发布 v版本号 发布内容时使用：同步版本号、验证、提交、创建同名 git 标签并推送提交和标签。
---

# Debug Comm Release

## Workflow

When the user asks `发布 v<version> <release notes>`, run the release flow in the `debug-comm` repository.

1. Parse `<version>` including the leading `v`, and use the version without `v` in project files.
2. Update these version fields:
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
   - the `debug-comm` package entry in `src-tauri/Cargo.lock`
3. Keep existing user changes intact. Do not revert unrelated work.
4. Validate with:
   - `pnpm build`
   - `git diff --check`
5. Commit the release with message `release: <version>`.
6. Create an annotated tag named `<version>` with message `<release notes>`.
7. Push the current branch and the tag to the configured remote.

## Notes

- If the tag already exists locally or remotely, stop and report the conflict instead of overwriting it.
- If validation fails, stop before committing or tagging and report the failing command.
- Include the release notes as the tag annotation body exactly as provided by the user after the version.
