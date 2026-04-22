"""hindsight-agent documents — list documents retained for an agent."""

from __future__ import annotations

import json

import click

from ..api import HindsightAPI
from ..config import get_agent


@click.command("documents")
@click.argument("agent_id")
def documents(agent_id: str) -> None:
    """List documents retained for an agent.

    Shows reference documents, conversation transcripts, and other
    content that has been retained into the agent's memory bank.
    """
    cfg = get_agent(agent_id)
    api = HindsightAPI(cfg.api_url, api_token=cfg.api_token)
    docs = api.list_documents(cfg.bank_id)
    click.echo(json.dumps({"documents": docs}, indent=2))
