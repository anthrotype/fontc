# Contributing to ttx-diff

Thank you for your interest in contributing to ttx-diff!

## Development Setup

### Prerequisites

- Python 3.10 or higher
- `uv` (recommended) or `pip`
- Git

### Setting Up Development Environment

1. Clone the repository:

```bash
git clone https://github.com/googlefonts/fontc.git
cd fontc/ttx_diff
```

2. Create and activate a virtual environment using `uv`:

```bash
uv venv
source .venv/bin/activate  # On Windows: .venv\Scripts\activate
```

3. Install the package in editable mode with development dependencies:

```bash
uv pip install -e ".[dev]"
```

Or using pip:

```bash
pip install -e ".[dev]"
```

## Running Tests

Run all tests:

```bash
pytest
```

Run tests with coverage:

```bash
pytest --cov=ttx_diff --cov-report=html
```

Run specific test file:

```bash
pytest tests/test_cli.py
```

Run tests in verbose mode:

```bash
pytest -v
```

## Code Style

We follow Python best practices:

- Use meaningful variable names
- Write docstrings for functions and classes
- Keep functions focused and small
- Add type hints where appropriate

## Testing Guidelines

- Write tests for new features
- Ensure all tests pass before submitting PR
- Aim for good test coverage
- Test edge cases and error conditions

## Submitting Changes

1. Create a new branch for your changes:

```bash
git checkout -b feature/your-feature-name
```

2. Make your changes and commit:

```bash
git add .
git commit -m "Description of your changes"
```

3. Push to your fork and create a pull request:

```bash
git push origin feature/your-feature-name
```

4. Ensure CI tests pass on your PR

## Release Process

Releases are managed through Git tags. To create a new release:

1. Update version in `pyproject.toml` and `src/ttx_diff/__init__.py`
2. Create and push a tag:

```bash
git tag -a ttx-diff-v0.1.0 -m "Release version 0.1.0"
git push origin ttx-diff-v0.1.0
```

3. GitHub Actions will automatically build and publish to PyPI

## Questions?

If you have questions, please open an issue on GitHub.
