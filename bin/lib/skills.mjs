import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const STOP = new Set([
  "a", "an", "the", "and", "or", "of", "for", "to", "in", "on", "with", "from",
  "by", "at", "as", "is", "are", "be", "this", "that", "into", "via", "plus",
]);

function tokenize(s) {
  return String(s || "")
    .toLowerCase()
    .split(/[^a-z0-9+.-]+/)
    .map((t) => t.replace(/^[-.]+|[-.]+$/g, ""))
    .filter((t) => t && t.length >= 2 && !STOP.has(t));
}

function parseSkillMd(text) {
  let description = "";
  let heading = "";
  const fm = String(text || "").match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (fm) {
    const d = fm[1].match(/^description:\s*(.*)$/m);
    if (d) {
      description = d[1].trim().replace(/^['"]|['"]$/g, "");
    }
  }
  const h = String(text || "").match(/^#\s+(.+)$/m);
  if (h) heading = h[1].trim();
  return { description, heading };
}

export function loadSkillCatalog(skillsDir) {
  const skills = [];
  if (!existsSync(skillsDir)) return skills;
  for (const id of readdirSync(skillsDir)) {
    const p = join(skillsDir, id, "SKILL.md");
    try {
      if (!existsSync(p) || !statSync(p).isFile()) continue;
    } catch {
      continue;
    }
    const text = readFileSync(p, "utf8");
    const meta = parseSkillMd(text);
    skills.push({ id, description: meta.description, heading: meta.heading });
  }
  return skills;
}

function tokenHit(queryTok, hayTok) {
  if (queryTok === hayTok) return true;
  if (queryTok.length >= 4 && hayTok.length >= 4) {
    if (hayTok.startsWith(queryTok) || queryTok.startsWith(hayTok)) return true;
  }
  return false;
}

function scoreOne(query, skill) {
  const qTokens = tokenize(query);
  const qLower = String(query || "").toLowerCase();
  const idTokens = tokenize(skill.id.replace(/-/g, " ") + " " + skill.id);
  const descTokens = tokenize(skill.description);
  const headTokens = tokenize(skill.heading);
  let score = 0;
  const hits = [];
  if (qLower.includes(skill.id)) {
    score += 5;
    hits.push("id:" + skill.id);
  }
  for (const qt of qTokens) {
    if (idTokens.some((ht) => tokenHit(qt, ht))) {
      score += 3;
      hits.push("id-token:" + qt);
      continue;
    }
    if (descTokens.some((ht) => tokenHit(qt, ht))) {
      score += 2;
      hits.push("desc:" + qt);
      continue;
    }
    if (headTokens.some((ht) => tokenHit(qt, ht))) {
      score += 1;
      hits.push("heading:" + qt);
    }
  }
  return { id: skill.id, score, hits };
}

export function rankSkills(query, catalog) {
  const ranked = catalog.map((s) => scoreOne(query, s));
  ranked.sort((a, b) => b.score - a.score || a.id.localeCompare(b.id));
  return ranked;
}

export function pickSkill(query, catalog) {
  const ranked = rankSkills(query, catalog);
  if (!ranked.length) {
    return {
      id: "explore",
      score: 0,
      hits: [],
      reason: "default explore (empty catalog)",
      alternatives: [],
    };
  }
  const best = ranked[0];
  const useDefault = best.score <= 0;
  const winner = useDefault
    ? ranked.find((r) => r.id === "explore") || best
    : best;
  const reason = useDefault
    ? "default explore (no description overlap)"
    : "matched " + (winner.hits.slice(0, 6).join(", ") || winner.id);
  const alternatives = ranked
    .filter((r) => r.id !== winner.id)
    .slice(0, 3)
    .map((r) => ({ skill: r.id, score: r.score }));
  return {
    id: winner.id,
    score: useDefault ? 0 : winner.score,
    hits: winner.hits,
    reason,
    alternatives,
  };
}
