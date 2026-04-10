## Feature Description
Brief description of the feature implemented

## Implementation Plan Confirmation
- [ ] Planning phase completed and reviewed
- [ ] Technical approach approved
- [ ] All requirements addressed
- [ ] Skill level verified (see .agents/skills/feature-development/SKILL.md)

## Changes Made
- List of key changes and files modified
- New components added (if any)
- Breaking changes (if any)

## Testing
- [ ] Unit tests implemented and passing
- [ ] Integration tests implemented and passing  
- [ ] Manual testing completed
- [ ] Edge cases tested

## Quality Gates Verification
- [ ] Code builds without warnings: `golem build`
- [ ] Code is properly formatted: `cargo fmt --all --check`
- [ ] Code passes clippy linting: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] All tests pass: `cargo test`

## Documentation Updates
- [ ] README updated for user-facing changes
- [ ] Code comments added for complex logic
- [ ] API documentation updated
- [ ] Configuration examples updated

## Checklist
- [ ] Feature follows the implementation workflow (`.windsurf/workflows/feature-implementation.md`)
- [ ] No hardcoded values or temporary solutions
- [ ] Error handling implemented appropriately
- [ ] Logging added where necessary
- [ ] Backward compatibility maintained
- [ ] Security considerations addressed

## Testing Commands Used
```bash
# Quality gate verification
make quality-check

# Individual checks
golem build
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
