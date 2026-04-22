"""hindsight-agent recall — query memories for an agent."""

from __future__ import annotations

import json

import click

from ..api import HindsightAPI
from ..config import get_agent


@click.command()
@click.argument("agent_id")
@click.argument("query")
@click.option("--max-results", "-n", default=10, help="Maximum results to return")
@click.option(
    "--type", "types", multiple=True,
    help="Filter by fact type (world, experience, observation). Repeatable.",
)
def recall(agent_id: str, query: str, max_results: int, types: tuple[str, ...]) -> None:
    """Recall memories for an agent.

    AGENT_ID identifies which agent's bank to query.
    QUERY is the natural language search query.
    """
    cfg = get_agent(agent_id)
    api = HindsightAPI(cfg.api_url)
    result = api.recall(
        cfg.bank_id,
        query,
        max_results=max_results,
        types=list(types) if types else None,
    )
    click.echo(json.dumps(result, indent=2))
