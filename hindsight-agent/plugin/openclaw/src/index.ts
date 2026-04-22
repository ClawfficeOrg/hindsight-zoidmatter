/**
 * Lightweight OpenClaw plugin for Hindsight Agent.
 *
 * On every agent_end, reads the agent config from ~/.hindsight-agent/config.json
 * to resolve bank ID and API URL, then POSTs filtered messages to Hindsight.
 *
 * No child_process. The config file is the single source of truth,
 * written by `hindsight-agent setup`.
 */

import { readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

interface PluginAPI {
  config: any;
  on(event: string, handler: (event: any, ctx?: any) => void | Promise<void>): void;
  logger: {
    info(msg: string): void;
    warn(msg: string): void;
    error(msg: string): void;
  };
}

interface AgentContext {
  agentId?: string;
  sessionKey?: string;
  workspaceDir?: string;
}

interface Message {
  role: string;
  content: string | any[];
}

interface AgentConfig {
  bank_id: string;
  api_url: string;
  api_token?: string;
}

const CONFIG_PATH = join(homedir(), ".hindsight-agent", "config.json");

function loadAgentConfig(agentId: string): AgentConfig | null {
  try {
    const raw = readFileSync(CONFIG_PATH, "utf-8");
    const config = JSON.parse(raw);
    return config.agents?.[agentId] ?? null;
  } catch {
    return null;
  }
}

const ALLOWED_ROLES = new Set(["user", "assistant"]);

function filterMessages(messages: Message[]): Array<{ role: string; content: string }> {
  const result: Array<{ role: string; content: string }> = [];

  for (const msg of messages) {
    const role = msg.role ?? "unknown";
    if (!ALLOWED_ROLES.has(role)) continue;

    let text = "";
    if (typeof msg.content === "string") {
      text = msg.content;
    } else if (Array.isArray(msg.content)) {
      text = msg.content
        .filter((block: any) => block?.type === "text" && block.text)
        .map((block: any) => block.text)
        .join("\n");
    }

    if (!text.trim()) continue;
    result.push({ role, content: text });
  }

  return result;
}

export default function (api: PluginAPI) {
  const log = api.logger;

  api.on("agent_end", async (event: any, ctx?: AgentContext) => {
    const agentId = ctx?.agentId;
    if (!agentId || !event?.success) return;

    const agentConfig = loadAgentConfig(agentId);
    if (!agentConfig) return;

    const messages: Message[] =
      event.context?.sessionEntry?.messages ?? event.messages ?? [];
    if (!messages.length) return;

    const filtered = filterMessages(messages);
    if (!filtered.length) return;

    const content = JSON.stringify(filtered);
    const sessionId = event.sessionKey ?? ctx?.sessionKey;
    const documentId = sessionId ? `${agentId}:${sessionId}` : undefined;

    const item: Record<string, any> = { content };
    if (documentId) item.document_id = documentId;

    const url = `${agentConfig.api_url}/v1/default/banks/${agentConfig.bank_id}/memories`;

    try {
      const response = await fetch(url, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(agentConfig.api_token ? { Authorization: `Bearer ${agentConfig.api_token}` } : {}),
        },
        body: JSON.stringify({ items: [item], async: true }),
        signal: AbortSignal.timeout(30_000),
      });

      if (!response.ok) {
        const body = await response.text().catch(() => "");
        log.error(`[hindsight-agent] retain failed (${response.status}): ${body}`);
        return;
      }

      log.info(`[hindsight-agent] retained ${filtered.length} messages for ${agentId}`);
    } catch (err: any) {
      log.error(`[hindsight-agent] retain error: ${err.message}`);
    }
  });
}
