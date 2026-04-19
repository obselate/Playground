"""CLI smoke tests.

We drive the CLI through :func:`regex_garden.cli.main` with captured stdout
and stderr rather than as a subprocess. This keeps the tests fast and works
whether or not the package has been installed.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from regex_garden.cli import main


def test_plant_command_prints_plant(capsys):
    rc = main(["plant", "abc"])
    captured = capsys.readouterr()
    assert rc == 0
    assert "a" in captured.out
    assert "~" in captured.out  # ground line


def test_plant_command_invalid_regex_returns_nonzero(capsys):
    rc = main(["plant", "("])
    captured = capsys.readouterr()
    assert rc == 2
    assert "invalid regex" in captured.err


def test_plant_command_with_label_prints_caption(capsys):
    rc = main(["plant", "--label", "greeting", "hi"])
    captured = capsys.readouterr()
    assert rc == 0
    assert "greeting" in captured.out


def test_examples_prints_multiple_plants(capsys):
    rc = main(["examples"])
    captured = capsys.readouterr()
    assert rc == 0
    # At least a few of the example names should appear as headings.
    assert "hello" in captured.out
    assert "alternation" in captured.out


def test_garden_command_reads_file(tmp_path: Path, capsys):
    garden_file = tmp_path / "mini.garden"
    garden_file.write_text(
        "- name: only\n"
        "  pattern: ab\n",
        encoding="utf-8",
    )
    rc = main(["garden", str(garden_file)])
    captured = capsys.readouterr()
    assert rc == 0
    assert "only" in captured.out


def test_garden_command_missing_file(capsys):
    rc = main(["garden", "/nonexistent/path/to.garden"])
    captured = capsys.readouterr()
    assert rc == 2
    assert "no such file" in captured.err


def test_garden_command_bad_entry_reports_invalid_regex(tmp_path: Path, capsys):
    bad = tmp_path / "bad.garden"
    bad.write_text(
        "- name: broken\n"
        "  pattern: (unclosed\n",
        encoding="utf-8",
    )
    rc = main(["garden", str(bad)])
    captured = capsys.readouterr()
    assert rc == 1
    assert "broken" in captured.err


def test_no_subcommand_errors(capsys):
    with pytest.raises(SystemExit):
        main([])
