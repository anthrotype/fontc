# ttx-diff

A tool for comparing font compiler outputs between fontc (Rust) and fontmake (Python).

## Overview

`ttx-diff` is a helper utility that compares binary font outputs from two different font compilers:
- **fontc**: The Rust-based font compiler from Google Fonts
- **fontmake**: The Python-based font compiler

The tool converts each binary font to TTX (XML) format, normalizes expected differences, and provides a detailed comparison summary.

## Installation

### From PyPI

```bash
pip install ttx-diff
```

### From source

```bash
git clone https://github.com/googlefonts/fontc.git
cd fontc/ttx_diff
pip install -e .
```

## Requirements

- Python 3.10 or higher
- `fontmake` must be installed and available in your environment
- `ttx` (from fonttools) must be available
- For fontc comparisons: `fontc` and `otl-normalizer` binaries

### Getting fontc and otl-normalizer

The tool needs the `fontc` and `otl-normalizer` binaries. You can:

1. **Specify paths explicitly** (recommended for most users):
   ```bash
   ttx-diff --fontc_path /path/to/fontc --normalizer_path /path/to/otl-normalizer source.glyphs
   ```

2. **Add them to your PATH**: If `fontc` and `otl-normalizer` are in your PATH, they'll be found automatically

3. **Run from fontc repository**: If you run from the fontc repository root, the tool will automatically build the binaries for you

## Usage

**Note**: Unlike the original script, this standalone version can be run from any directory. You don't need to be in the fontc repository.

### Basic comparison

Rebuild with both fontmake and fontc and compare:

```bash
ttx-diff --fontc_path /path/to/fontc --normalizer_path /path/to/otl-normalizer path/to/source.glyphs
```

If the binaries are in your PATH:

```bash
ttx-diff path/to/source.glyphs
```

### Selective rebuild

Rebuild only fontc and reuse existing fontmake output:

```bash
ttx-diff --rebuild fontc path/to/source.glyphs
```

### JSON output

Output results in machine-readable JSON format:

```bash
ttx-diff --json path/to/source.glyphs
```

### Using gftools

Compare using gftools build pipeline:

```bash
ttx-diff --compare gftools --config config.yaml path/to/source.glyphs
```

## Command-line Options

- `--config`: Path to config.yaml for gftools mode
- `--fontc_path`: Path to precompiled fontc binary
- `--normalizer_path`: Path to precompiled otl-normalizer binary
- `--compare`: Comparison mode (`default` or `gftools`)
- `--rebuild`: Which compilers to rebuild (`both`, `fontc`, `fontmake`, `none`)
- `--json`: Output results in JSON format
- `--outdir`: Directory to store generated files
- `--production_names`: Rename glyphs to production names (default: True)
- `--keep_overlaps`: Keep overlaps when building static fonts (default: True)
- `--keep_direction`: Preserve contour winding direction from source
- `--off_by_one_budget`: Percentage of values allowed to differ by one (default: 0.1)

## JSON Output Format

When `--json` is specified, the tool outputs a JSON object:

### Success case

```json
{
  "success": {
    "GPOS": 0.99,
    "vmtx": "fontmake",
    "total": 0.98
  }
}
```

- Keys are table tags or identifiers
- Values are either:
  - A float (0.0-1.0) representing similarity ratio (1.0 = identical)
  - A string indicating which compiler produced that table exclusively

### Error case

```json
{
  "error": {
    "fontc": {
      "command": "fontc --build-dir . -o fontc.ttf source.glyphs",
      "stderr": "error message..."
    }
  }
}
```

## Exit Codes

- `0`: Outputs are identical
- `2`: Outputs differ or a compiler failed

## Development

### Running tests

```bash
pytest
```

### Running tests with coverage

```bash
pytest --cov=ttx_diff --cov-report=html
```

## License

Apache License 2.0

## Contributing

Contributions are welcome! Please open an issue or pull request on the [GitHub repository](https://github.com/googlefonts/fontc).
