# Feature Implementation Workflow

This workflow ensures consistent, high-quality feature implementation through a structured 3-phase process.

## Phase 1: Planning (Required First Step)

### 1.1 Feature Analysis
- Define clear feature requirements and acceptance criteria
- Identify affected components and dependencies
- Analyze impact on existing functionality
- Estimate complexity and potential risks

### 1.2 Technical Planning
- Design the technical approach and architecture
- Identify required changes to data models, APIs, or configurations
- Plan testing strategy (unit tests, integration tests, manual testing)
- Consider backward compatibility and migration needs

### 1.3 Implementation Plan
- Break down work into specific, actionable tasks
- Identify files that need modification or creation
- Plan order of implementation to minimize breaking changes
- Define success criteria and verification methods

## Phase 2: Confirmation (Required Before Implementation)

### 2.1 Plan Review
- Present the complete plan for review
- Verify all requirements are addressed
- Confirm technical approach is sound
- Check for potential edge cases or missing considerations

### 2.2 Risk Assessment
- Identify potential blockers or challenges
- Plan mitigation strategies for identified risks
- Confirm resource availability and timelines
- Get explicit approval to proceed with implementation

### 2.3 Implementation Readiness Check
- Confirm all dependencies are available
- Verify development environment is properly set up
- Ensure understanding of existing codebase patterns
- Commit to quality standards (buildable, formatted, linted code)

## Phase 3: Implementation (With Quality Gates)

### 3.1 Core Implementation
- Implement changes following the approved plan
- Follow existing code patterns and conventions
- Write code that is self-documenting and maintainable
- Add appropriate error handling and logging

### 3.2 Testing Implementation
- Implement unit tests for new functionality
- Add integration tests where applicable
- Test edge cases and error conditions
- Verify backward compatibility

### 3.3 Quality Gates (Must Pass Before Considered "Done")
- **Buildable**: Code compiles without errors or warnings
- **Formatted**: Code follows project formatting standards (`cargo fmt --all`)
- **Linted**: Code passes all linting checks (`cargo clippy --all-targets --all-features -- -D warnings`)
- **Tests Pass**: All tests succeed (`cargo test`)

### 3.4 Documentation Updates
- Update relevant documentation (README, API docs, etc.)
- Add code comments for complex logic
- Update configuration examples if needed
- Document any breaking changes

## Quality Standards

### Code Quality Requirements
- Code must compile without any warnings
- Must pass `cargo fmt --all` formatting check
- Must pass `cargo clippy --all-targets --all-features -- -D warnings`
- All tests must pass: `cargo test`

### Testing Requirements
- Unit tests for all new functions/methods
- Integration tests for new API endpoints
- Error path testing
- Edge case coverage

### Documentation Requirements
- Clear commit messages following conventional format
- Updated README for user-facing changes
- Code comments for complex business logic
- API documentation for new endpoints

## Skills & Prerequisites

Before starting the feature implementation workflow, ensure you have the required skills:

- **Beginner**: Can follow detailed plans and run quality gates
- **Intermediate**: Can implement features independently
- **Advanced**: Can design architecture and lead development

See `.agents/skills/feature-development/SKILL.md` for complete skill requirements and development paths.

## Usage Commands

### Planning Phase
```bash
# Create feature branch
git checkout -b feature/your-feature-name

# Start planning (use this workflow)
# Document your plan using templates/feature-plan.md
```

### Quality Gate Commands
```bash
# Build check
golem build

# Format check
cargo fmt --all --check

# Lint check  
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test

# Complete quality gate check
make quality-check
```

## Templates

### Feature Plan Template
Use `templates/feature-plan.md` for structured planning.

### Pull Request Template
Use `.github/PULL_REQUEST_TEMPLATE.md` for consistent PRs.

## Automation Scripts

### Quality Check Script
Run `./scripts/quality-check.sh` for complete validation.

### Pre-commit Hook
Install with `make install-hooks` for automatic quality checks.

This workflow ensures every feature follows a consistent process with proper planning, confirmation, and quality assurance before being considered complete.
