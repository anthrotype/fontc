"""Tests for the ttx-diff CLI."""

import subprocess
import sys
from pathlib import Path
import pytest


def test_import():
    """Test that the package can be imported."""
    import ttx_diff
    assert ttx_diff.__version__ == "0.1.0"


def test_cli_no_args():
    """Test CLI with no arguments exits with error."""
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli"],
        capture_output=True,
        text=True,
    )
    # Should exit with error (not 0) when no arguments provided
    assert result.returncode != 0
    assert "Only one argument" in result.stderr or "USAGE" in result.stderr or "required" in result.stderr.lower()


def test_cli_help():
    """Test CLI help flag."""
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli", "--help"],
        capture_output=True,
        text=True,
    )
    # absl exits with 1 when showing help (not 0), this is expected behavior
    assert result.returncode in [0, 1]
    # Check for some expected help text
    assert any(
        keyword in result.stdout
        for keyword in ["fontc", "fontmake", "compare", "rebuild", "ttx_diff", "help"]
    )


def test_cli_missing_source():
    """Test CLI with non-existent source file."""
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli", "/nonexistent/file.glyphs"],
        capture_output=True,
        text=True,
    )
    # Should exit with error
    assert result.returncode != 0
    # Should mention the missing file
    assert "No such source" in result.stderr or "does not exist" in result.stderr.lower()


def test_cli_invalid_rebuild_option():
    """Test CLI with invalid --rebuild option."""
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli", "--rebuild", "invalid", "dummy.glyphs"],
        capture_output=True,
        text=True,
    )
    # Should exit with error due to invalid enum value
    assert result.returncode != 0


def test_cli_json_flag_with_missing_source():
    """Test that --json flag is recognized even with missing source."""
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli", "--json", "/nonexistent/file.glyphs"],
        capture_output=True,
        text=True,
    )
    # Should exit with error, but --json should be parsed
    assert result.returncode != 0


def test_cli_version_flags():
    """Test that various version-related flags work."""
    # Test --helpfull flag (from absl)
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli", "--helpfull"],
        capture_output=True,
        text=True,
    )
    # absl exits with 1 when showing help (not 0), this is expected behavior
    assert result.returncode in [0, 1]
    assert len(result.stdout) > 100  # Should have substantial help text


def test_main_function_import():
    """Test that main function can be imported."""
    from ttx_diff import main
    assert callable(main)


def test_core_module_import():
    """Test that core module can be imported."""
    from ttx_diff import core
    assert hasattr(core, "main")
    assert callable(core.main)


def test_cli_multiple_args_error():
    """Test CLI with multiple source arguments."""
    result = subprocess.run(
        [sys.executable, "-m", "ttx_diff.cli", "file1.glyphs", "file2.glyphs"],
        capture_output=True,
        text=True,
    )
    # Should exit with error - only one source expected
    assert result.returncode != 0


@pytest.mark.parametrize("compare_mode", ["default", "gftools"])
def test_cli_compare_modes(compare_mode):
    """Test that compare mode flags are parsed correctly."""
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "ttx_diff.cli",
            "--compare",
            compare_mode,
            "/nonexistent/file.glyphs",
        ],
        capture_output=True,
        text=True,
    )
    # Should fail on missing file, but compare flag should be accepted
    assert result.returncode != 0
    assert "No such source" in result.stderr or "does not exist" in result.stderr.lower()


@pytest.mark.parametrize("rebuild_mode", ["both", "fontc", "fontmake", "none"])
def test_cli_rebuild_modes(rebuild_mode):
    """Test that rebuild mode flags are parsed correctly."""
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "ttx_diff.cli",
            "--rebuild",
            rebuild_mode,
            "/nonexistent/file.glyphs",
        ],
        capture_output=True,
        text=True,
    )
    # Should fail on missing file, but rebuild flag should be accepted
    assert result.returncode != 0
    assert "No such source" in result.stderr or "does not exist" in result.stderr.lower()


def test_cli_float_flag():
    """Test that float flag (off_by_one_budget) is parsed correctly."""
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "ttx_diff.cli",
            "--off_by_one_budget",
            "0.2",
            "/nonexistent/file.glyphs",
        ],
        capture_output=True,
        text=True,
    )
    # Should fail on missing file, but float flag should be accepted
    assert result.returncode != 0


def test_cli_bool_flags():
    """Test that boolean flags are parsed correctly."""
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "ttx_diff.cli",
            "--noproduction_names",
            "--nokeep_overlaps",
            "/nonexistent/file.glyphs",
        ],
        capture_output=True,
        text=True,
    )
    # Should fail on missing file, but bool flags should be accepted
    assert result.returncode != 0


def test_helpful_error_for_missing_binaries(temp_dir, minimal_ufo):
    """Test that missing binaries produce helpful error messages."""
    # This test assumes fontc is not in PATH (which is likely in CI)
    # We test that the error message is helpful
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "ttx_diff.cli",
            str(minimal_ufo),
        ],
        capture_output=True,
        text=True,
        cwd=str(temp_dir),  # Run from temp dir, not fontc repo
    )
    # Should exit with error
    assert result.returncode != 0
    # Should mention one of the missing binaries or fontmake
    assert any(
        keyword in result.stderr
        for keyword in ["fontc", "otl-normalizer", "fontmake", "Could not find"]
    )
