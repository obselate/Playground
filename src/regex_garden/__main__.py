"""Allow ``python -m regex_garden`` to invoke the CLI."""

from regex_garden.cli import main


if __name__ == "__main__":
    raise SystemExit(main())
