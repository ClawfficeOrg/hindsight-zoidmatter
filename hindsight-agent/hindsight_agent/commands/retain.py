"""hindsight-agent retain — called by harness plugins to retain conversation content."""

from __future__ import annotations

import json
import sys

import click

from ..api import HindsightAPI
from ..config import get_agent


@click.command()
@click.argument("agent_id")
@click.option(
    "--input",
    "input_file",
    type=click.Path(exists=True),
    default=None,
    help="Path to file with content to retain. Reads stdin if omitted.",
)
@click.option(
    "--document-id",
    default=None,
    help="Document ID for upsert behavior.",
)
def retain(agent_id: str, input_file: str | None, document_id: str | None) -> None:
    """Retain content for an agent.

    AGENT_ID identifies which agent's bank to retain into.
    Content is read as-is from --input or stdin and passed directly to Hindsight.
    The caller (plugin) decides the format.
    """
    cfg = get_agent(agent_id)
    api = HindsightAPI(cfg.api_url)

    # Read content as raw text
    if input_file:
        with open(input_file) as f:
            content = f.read()
    else:
        content = sys.stdin.read()

    if not content.strip():
        click.echo("No content to retain.", err=True)
        return

    result = api.retain(cfg.bank_id, content, document_id=document_id)
    click.echo(json.dumps(result))
