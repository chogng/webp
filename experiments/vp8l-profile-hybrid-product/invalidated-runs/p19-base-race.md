# P19 creation-base race

P19 was requested from local committed `main` at
`f4c4ae0b82851288d77a55b709bb9c0a3951ef50`. Its queued Codex worktree was
created at that commit, but local `main` advanced before the task began its
mandatory identity check.

- Task: `019f8a82-b5c3-7291-8406-883fdb7cdbdf`
- Queued client task: `client-new-thread:51dd06bd-224a-4594-b3a6-a1bca986a36f`
- Worktree: `/Users/lance/.codex/worktrees/4365/webp`
- Worktree HEAD: `f4c4ae0b82851288d77a55b709bb9c0a3951ef50`
- Worktree/main merge-base: `f4c4ae0b82851288d77a55b709bb9c0a3951ef50`
- Local main at preflight: `f0b5fd4d0bc4f65f80372cf1376f6feaa89bb6d9`
- Branch: none; the worktree remained detached

The task stopped without edits, commits, implementation, or measurements. No
performance result from P19 is valid. Product migration must use a newly
created worktree whose HEAD, local main, and merge-base are identical at its
own preflight.
