---
name: feature-development
description: "Comprehensive skill for implementing new features in the Golem RAG System, following a structured 3-phase workflow with planning, confirmation, and quality gates."
---

# Feature Development Skill

## Description
Comprehensive skill for implementing new features in the Golem RAG System, following a structured 3-phase workflow with planning, confirmation, and quality gates.

## Prerequisites
- Basic Rust programming knowledge
- Understanding of Git version control
- Familiarity with command-line tools

## Skill Levels

### Beginner (Contributor Level)
**Can complete with guidance:**
- Simple bug fixes and documentation updates
- Adding tests for existing functionality
- Following detailed implementation plans
- Running quality gate checks

### Intermediate (Feature Developer)
**Can complete independently:**
- Implement well-defined features
- Design and implement tests
- Follow the complete workflow process
- Maintain code quality standards

### Advanced (System Designer)
**Can lead complex work:**
- Design new components and architecture
- Define requirements and technical approach
- Mentor other contributors
- Improve development processes

## Core Capabilities

### Phase 1: Planning
- [ ] **Requirements Analysis**: Breaking down complex requirements
- [ ] **System Design**: Component interaction, data flow
- [ ] **Risk Assessment**: Identifying potential blockers
- [ ] **Task Breakdown**: Creating actionable implementation steps
- [ ] **Technical Writing**: Clear, comprehensive documentation
- [ ] **Template Usage**: Following structured templates

### Phase 2: Confirmation
- [ ] **Technical Communication**: Explaining complex concepts clearly
- [ ] **Peer Review**: Giving and receiving constructive feedback
- [ ] **Stakeholder Management**: Getting appropriate approvals
- [ ] **Consensus Building**: Aligning team on approach

### Phase 3: Implementation
- [ ] **Clean Code**: Readable, maintainable code
- [ ] **Pattern Following**: Consistent with existing codebase
- [ ] **Error Handling**: Robust error management
- [ ] **Logging**: Appropriate logging for debugging
- [ ] **Test Design**: Comprehensive test coverage
- [ ] **Test Automation**: Automated quality gates
- [ ] **Edge Case Testing**: Boundary conditions and error paths
- [ ] **Integration Testing**: Component interaction testing

## Technical Knowledge Areas

### Rust Programming
**Essential Skills:**
- [ ] **Rust Fundamentals**: Ownership, borrowing, lifetimes, traits
- [ ] **Async Programming**: tokio, async/await patterns
- [ ] **Error Handling**: Result, Option, anyhow, thiserror
- [ ] **Testing**: Unit tests, integration tests, mocks
- [ ] **Build System**: Cargo, workspaces, dependencies

**Golem-Specific Skills:**
- [ ] **Golem Cloud**: Component development, RPC calls
- [ ] **WASM Target**: `wasm32-wasip2` compilation
- [ ] **cargo-component**: Component building and deployment

### Database Skills
**PostgreSQL + pgvector:**
- [ ] **SQL**: Complex queries, joins, indexing
- [ ] **pgvector**: Vector operations, similarity search
- [ ] **Migrations**: Schema changes, data migration
- [ ] **Connection Pooling**: Database connection management

### S3 & Storage
**S3-Compatible Storage:**
- [ ] **S3 API**: Object storage operations
- [ ] **Authentication**: IAM roles, access keys
- [ ] **Multi-bucket**: Organization across buckets
- [ ] **Content Types**: File type detection and handling

### DevOps & Infrastructure
**Container & Deployment:**
- [ ] **Docker**: Containerization, docker-compose
- [ ] **Environment Management**: .env configuration
- [ ] **Shell Scripting**: Bash automation scripts
- [ ] **Git Hooks**: Pre-commit quality gates

## Quality Standards

### Code Quality Requirements
- [ ] **rustfmt**: Code formatting standards
- [ ] **clippy**: Linting rules and best practices
- [ ] **Warning-Free Code**: Zero compilation warnings
- [ ] **Documentation**: Code comments and API docs

### Build & Deployment
- [ ] **Build Automation**: Makefile usage and maintenance
- [ ] **CI/CD**: Continuous integration principles
- [ ] **Version Control**: Git best practices
- [ ] **Release Management**: Deployment procedures

## Domain-Specific Knowledge

### RAG (Retrieval-Augmented Generation)
**Vector Search:**
- [ ] **Embeddings**: Vector representations and similarity
- [ ] **Hybrid Search**: Combining semantic and keyword search
- [ ] **Reciprocal Rank Fusion**: Result combination algorithms
- [ ] **Search Relevance**: Ranking and threshold tuning

**Document Processing:**
- [ ] **Content Extraction**: PDF, HTML, markdown parsing
- [ ] **Text Chunking**: Document segmentation strategies
- [ ] **Metadata Management**: Document organization
- [ ] **Content Type Detection**: File type handling

### API Development
**REST API Design:**
- [ ] **HTTP Methods**: Proper RESTful design
- [ ] **JSON Handling**: Serialization/deserialization
- [ ] **Error Responses**: Consistent error formats
- [ ] **API Documentation**: Clear endpoint documentation

## Workflow Integration

### Commands & Tools
```bash
# Quality gate verification
make quality-check

# Individual quality gates
make build          # Build all components
make fmt-check      # Check formatting
make lint           # Run clippy
make test           # Run tests

# Format code
make fmt

# Pre-push validation
make pre-push
```

### Templates & Checklists
- **Feature Planning**: `templates/feature-plan.md`
- **PR Template**: `.github/PULL_REQUEST_TEMPLATE.md`
- **Workflow Guide**: `docs/feature-implementation-workflow.md`

### Quality Gates as Skill Verification
- **Build success** = Technical competence
- **Format compliance** = Attention to detail
- **Lint passing** = Code quality understanding
- **Test coverage** = Testing proficiency

## Learning Path

### For New Contributors
1. Start with documentation and simple fixes
2. Learn the codebase through existing components
3. Practice the workflow process
4. Progress to independent feature development

### For Skill Improvement
- Code reviews with experienced developers
- Study existing implementation patterns
- Contribute to increasingly complex features
- Participate in architecture discussions

## Onboarding Checklist

### Setup Requirements
- [ ] Development environment configured
- [ ] Quality gates installed and tested
- [ ] Workflow templates reviewed
- [ ] Codebase structure understood

### First Contribution
- [ ] Choose a beginner-appropriate task
- [ ] Follow the complete workflow process
- [ ] Request code review and feedback
- [ ] Reflect on lessons learned

This skill enables structured, high-quality feature development with proper planning, confirmation, and quality assurance processes.
