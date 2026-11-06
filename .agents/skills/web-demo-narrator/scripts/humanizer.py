"""
Speech Humanization Engine (R1) for Brazilian Portuguese (pt-BR) oral narration.
Adapts AI-generated scripts into natural spoken speech while strictly preserving
technical selectors, URLs, and code tokens via 3-stage nonce shielding.
"""

from __future__ import annotations

import copy
import re
from typing import Dict, List, Tuple, Optional, Any, Union
from .schema import DemoScript, Scene


class SpeechHumanizer:
    """Core engine for humanizing Portuguese oral speech and eliminating AI writing tells."""

    SHIELD_PREFIX = "___SHIELD_TOKEN_"
    SHIELD_SUFFIX = "___"

    # Regex patterns for technical entities to shield
    TECHNICAL_PATTERNS = [
        # Inline code blocks
        r"`[^`]+`",
        # Full URLs (http/https) - negative lookbehind ensures sentence punctuation is not swallowed
        r"https?://[^\s)\]>\"',]+(?<![.,;:!?])",
        # XPath selectors (//tag[@attr='val']...)
        r"//[a-zA-Z0-9_\-]+(?:\[[^\]]+\])*(?:/[a-zA-Z0-9_\-]+(?:\[[^\]]+\])*)*",
        # CSS selectors with attributes (e.g. input[name='search'], button[type="submit"])
        r"\b[a-zA-Z0-9_\-]+\[[^\]]+\]",
        # Standalone CSS attribute selectors (e.g. [data-testid="submit"])
        r"\[[a-zA-Z0-9_\-]+[~|^$*]?=[^\]]+\]",
        # CSS pseudo-classes
        r"\b[a-zA-Z0-9_\-]+:[a-zA-Z0-9_\-]+(?:\([^)]*\))?",
        # CSS compound selectors with ID or class (e.g. button#btn-salvar, div.card-body)
        r"\b[a-zA-Z0-9_\-]+#[a-zA-Z][\w\-]*\b",
        r"\b[a-zA-Z0-9_\-]+\.[a-zA-Z][\w\-]*\b",
        # Standalone CSS ID selectors (e.g. #btn-salvar, #filtro)
        r"(?<![\w\-.])#[a-zA-Z][\w\-]*\b",
        # Standalone CSS class selectors (e.g. .btn-primary, .active-tab)
        r"(?<![\w\-.])\.[a-zA-Z][\w\-]*\b",
        # Relative endpoints / paths with multiple segments (e.g. /v1/audio/speech, /dashboard/relatorios)
        r"/(?:[a-zA-Z0-9_\-]+/)+[a-zA-Z0-9_\.\-]+(?<![.,;:!?])",
        # CLI flags (e.g. --diff, --headless, --timeout)
        r"--[a-zA-Z0-9_\-]+",
        # Method / function calls (e.g. login(), handleClick())
        r"\b[a-zA-Z_][a-zA-Z0-9_]*\(\)",
        # Key technical acronyms and terms
        r"\b(?:JSON|REST|API|UUID|OAuth|HTML|CSS|DOM|FFmpeg|ffprobe|Playwright|CUDA|WAV|MP4|H\.264|AAC|PCM|RTX\s*4060)\b",
    ]

    def __init__(self):
        self._combined_tech_regex = re.compile(
            "|".join(f"(?:{p})" for p in self.TECHNICAL_PATTERNS)
        )

    def mask_technical_tokens(self, text: str) -> Tuple[str, Dict[str, str]]:
        """Stage 1: Extract technical entities and replace with unique nonce tokens."""
        token_map: Dict[str, str] = {}
        counter = 0

        def replace_match(match: re.Match) -> str:
            nonlocal counter
            original = match.group(0)
            token = f"{self.SHIELD_PREFIX}{counter}{self.SHIELD_SUFFIX}"
            token_map[token] = original
            counter += 1
            return token

        masked_text = self._combined_tech_regex.sub(replace_match, text)
        return masked_text, token_map

    def unmask_technical_tokens(self, text: str, token_map: Dict[str, str]) -> str:
        """Stage 3: Restore all technical tokens with exact character fidelity."""
        result = text
        for token, original in token_map.items():
            result = result.replace(token, original)

        # Invariant verification: ensure no active shield tokens remain
        for token in token_map:
            if token in result:
                raise ValueError(f"Integrity error: Unrestored shield token detected in text: {token}")
        if self.SHIELD_PREFIX in result:
            raise ValueError(f"Integrity error: Unrestored shield token detected in text: {result}")

        return result

    def eliminate_ai_patterns(self, text: str) -> str:
        """Stage 2A: Remove typical LLM prose clichés and artificial structures."""
        # 1. Chatbot residue (§22)
        residue_patterns = [
            r"(?i)\b(?:espero\s+que\s+(?:isso\s+)?ajude|espero\s+ter\s+ajudado)[.!]*\s*",
            r"(?i)\b(?:com\s+certeza|certamente|com\s+toda\s+certeza)[.!]*\s*",
            r"(?i)\b(?:ótima\s+pergunta|excelente\s+pergunta)[.!]*\s*",
            r"(?i)\bvocê\s+tem\s+toda\s+razão[.!]*\s*",
            r"(?i)\b(?:qualquer\s+dúvida|se\s+precisar\s+de\s+(?:mais\s+)?algo),\s*(?:estou\s+à\s+disposição|é\s+só\s+chamar|me\s+avise)[.!]*\s*",
        ]
        for pat in residue_patterns:
            text = re.sub(pat, "", text)

        # 2. Dramatic closers and fragments (§2) - strictly anchored to clause/sentence boundaries
        closer_patterns = [
            (r"(?i)\bno\s+final\s+das\s+contas,\s*", "Na prática, "),
            (r"(?i),\s*no\s+final\s+das\s+contas(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\bno\s+final\s+das\s+contas(?:[.!?]+(?:\s+|$)|$)", ""),
            (r"(?i),\s*simples\s+assim(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\bsimples\s+assim(?:[.!?]+(?:\s+|$)|$)", ""),
            (r"(?i),\s*e\s+é\s+isso(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\be\s+é\s+isso(?:[.!?]+(?:\s+|$)|$)", ""),
            (r"(?i),\s*esse\s+é\s+o\s+(?:grande\s+)?diferencial(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\besse\s+é\s+o\s+(?:grande\s+)?diferencial(?:[.!?]+(?:\s+|$)|$)", ""),
            (r"(?i),\s*essa\s+é\s+a\s+(?:verdadeira\s+)?virada\s+de\s+chave(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\bessa\s+é\s+a\s+(?:verdadeira\s+)?virada\s+de\s+chave(?:[.!?]+(?:\s+|$)|$)", ""),
            (r"(?i),\s*isso\s+muda\s+tudo(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\bisso\s+muda\s+tudo(?:[.!?]+(?:\s+|$)|$)", ""),
            (r"(?i),\s*ponto\s+final(?:[.!?]+(?:\s+|$)|$)", "."),
            (r"(?i)(?:(?<=[.!?]\s)|^)\bponto\s+final(?:[.!?]+(?:\s+|$)|$)", ""),
        ]
        for pat, repl in closer_patterns:
            text = re.sub(pat, repl, text)

        # 3. Staged run-ups (§4) - preserve definite articles and grammatical gender
        runup_patterns = [
            (r"(?i)\bvamos\s+mergulhar\s+no\s+", "Vejamos o "),
            (r"(?i)\bvamos\s+mergulhar\s+na\s+", "Vejamos a "),
            (r"(?i)\bvamos\s+mergulhar\s+nos\s+", "Vejamos os "),
            (r"(?i)\bvamos\s+mergulhar\s+nas\s+", "Vejamos as "),
            (r"(?i)\bvamos\s+mergulhar\s+(?:em\s+)?", "Vejamos "),
            (r"(?i)\bvamos\s+explorar\s+(o|a|os|as)\s+", r"Vejamos \1 "),
            (r"(?i)\bvamos\s+explorar\s+", "Vejamos "),
            (r"(?i)\baqui\s+está\s+o\s+que\s+você\s+precisa\s+saber:?\s*", ""),
            (r"(?i)\bsem\s+mais\s+delongas,\s*", ""),
            (r"(?i)\bagora\s+vamos\s+(?:ver\s+como|entender\s+como)\s+", "Veja como "),
        ]
        for pat, repl in runup_patterns:
            text = re.sub(pat, repl, text)

        # 4. Aphorisms & Deep talk (§3)
        aphorism_patterns = [
            (r"(?i)\bno\s+coração\s+(?:do|da|de)\s+", "Aqui no "),
            (r"(?i)\ba\s+verdadeira\s+questão\s+é\s+", "A questão é "),
            (r"(?i)\bo\s+cerne\s+da\s+questão\s+é\s+", "O ponto principal é "),
            (r"(?i)\bno\s+fim\s+do\s+dia,\s*", "Na prática, "),
            (r"(?i)\bfundamentalmente,\s*", "Na prática, "),
        ]
        for pat, repl in aphorism_patterns:
            text = re.sub(pat, repl, text)

        # 5. Arguing with nobody (§5)
        arguing_patterns = [
            (r"(?i)\bisso\s+não\s+quer\s+dizer\s+que\s+", "Vale notar que "),
            (r"(?i)\bnão\s+me\s+entenda\s+mal,\s*", ""),
            (r"(?i)\bpara\s+deixar\s+claro,\s*", "Vale ressaltar que "),
        ]
        for pat, repl in arguing_patterns:
            text = re.sub(pat, repl, text)

        # 6. Not X but Y (§1)
        # Preserve copular verb "é" when present, bound to sentence delimiters
        sep = r",\s*(?:mas\s+(?:também|sim|como\s+também)?|como\s+também)\s*"
        not_x_patterns = [
            (rf"(?i)\bnão\s+é\s+(?:apenas|só|somente)\s+([^,.!?]+?){sep}([^.!?]+)", r"é \1 e \2"),
            (rf"(?i)\bnão\s+(?:apenas|só|somente)\s+([^,.!?]+?){sep}([^.!?]+)", r"\1 e \2"),
        ]
        for pat, repl in not_x_patterns:
            text = re.sub(pat, repl, text)

        # 7. Forced Triads (§6) - curated rhetorical triples (avoiding deletion of factual lists)
        triad_patterns = [
            (r"(?i)\brápido,\s*prático\s+e\s+intuitivo\b", "rápido e prático"),
            (r"(?i)\brápido,\s*seguro\s+e\s+intuitivo\b", "seguro e direto"),
            (r"(?i)\bsimples,\s*rápido\s+e\s+fácil\b", "simples e direto"),
            (r"(?i)\bsimples,\s*ágil\s+e\s+eficiente\b", "simples e eficiente"),
            (r"(?i)\brobusto,\s*escalável\s+e\s+seguro\b", "seguro e escalável"),
            (r"(?i)\binovação,\s*inspiração\s+e\s+insights\b", "inovação e ideias"),
            (r"(?i)\bmoderno,\s*elegante\s+e\s+sofisticado\b", "moderno e prático"),
            (r"(?i)\bágil,\s*moderno\s+e\s+escalável\b", "ágil e moderno"),
            (r"(?i)\bfácil,\s*intuitivo\s+e\s+transparente\b", "fácil e transparente"),
            (r"(?i)\bdinâmico,\s*flexível\s+e\s+poderoso\b", "dinâmico e flexível"),
            (r"(?i)\bcompleto,\s*integrado\s+e\s+confiável\b", "completo e integrado"),
            (r"(?i)\bvisual,\s*interativo\s+e\s+agradável\b", "visual e interativo"),
        ]
        for pat, repl in triad_patterns:
            text = re.sub(pat, repl, text)

        # 8. Dash replacement (§8)
        # Convert em-dash (—), en-dash (–), and double hyphen (--) into commas/periods
        text = re.sub(r"\s*[—–]\s*", ", ", text)
        text = re.sub(r"\s+--\s+", ", ", text)
        text = re.sub(r"--", ", ", text)

        # 9. AI Clichés & Overused vocabulary (§12 & §13)
        cliche_replacements = [
            (r"\brobustos\b", "estáveis"),
            (r"\brobustas\b", "estáveis"),
            (r"\brobusto\b", "estável"),
            (r"\brobusta\b", "estável"),
            (r"\bRobusto\b", "Estável"),
            (r"\bRobusta\b", "Estável"),
            (r"\becossistema\b", "ambiente"),
            (r"\bEcossistema\b", "Ambiente"),
            (r"\bcruciais\b", "importantes"),
            (r"\bcrucial\b", "importante"),
            (r"\bCrucial\b", "Importante"),
            (r"\bpivotal\b", "central"),
            (r"\bmeticulosamente\b", "com cuidado"),
            (r"\brevolucionário\b", "moderno"),
            (r"\brevolucionária\b", "moderna"),
            (r"\btestemunho\b", "demonstração"),
            (r"\buma tapeçaria de\b", "um conjunto de"),
            (r"\belevar para o próximo nível\b", "aprimorar"),
        ]
        for pat, repl in cliche_replacements:
            text = re.sub(pat, repl, text)

        return text

    def apply_oral_cadence(self, text: str) -> str:
        """Stage 2B: Adjust breathing rhythm and chunk long clauses (>16 words)."""
        # Clean double commas and redundant spaces first
        text = re.sub(r",\s*,+", ", ", text)
        text = re.sub(r"\s+", " ", text).strip()

        # Split into sentences while keeping punctuation
        sentences = re.split(r"([.!?]+(?:\s+|$))", text)
        reconstructed: List[str] = []

        conjunction_words = {
            "que", "onde", "quando", "para", "porque", "pois", "com", "ao", "e",
            "enquanto", "já que", "assim que", "visto que"
        }

        for part in sentences:
            if not part:
                continue
            # If it's punctuation, keep as is
            if re.match(r"^[.!?]+\s*$", part):
                reconstructed.append(part)
                continue

            # Ensure sentence begins with uppercase
            s_stripped = part.lstrip()
            leading_space = part[: len(part) - len(s_stripped)]
            if s_stripped and s_stripped[0].islower():
                part = leading_space + s_stripped[0].upper() + s_stripped[1:]

            # Split sentence into existing punctuation clauses
            clauses = re.split(r"([,;:…]+)", part)
            processed_clauses: List[str] = []

            for i in range(0, len(clauses), 2):
                clause = clauses[i]
                punct = clauses[i + 1] if i + 1 < len(clauses) else ""
                words = clause.strip().split()

                if len(words) > 16:
                    # Clause exceeds 16 words; insert natural breathing comma
                    # Search around the middle (words 8 to 14) for a natural conjunction
                    insert_idx = -1
                    min_search = 7
                    max_search = min(14, len(words) - 2)

                    for w_i in range(min_search, max_search + 1):
                        clean_word = words[w_i].lower().strip(",.;:")
                        if clean_word in conjunction_words:
                            insert_idx = w_i
                            break

                    if insert_idx == -1:
                        # Fallback: break at word 10
                        insert_idx = min(10, len(words) - 2)

                    first_half = " ".join(words[:insert_idx])
                    second_half = " ".join(words[insert_idx:])
                    clause = f"{first_half}, {second_half}"

                processed_clauses.append(clause + punct)

            reconstructed.append("".join(processed_clauses))

        result = "".join(reconstructed)

        # Clean up any artifacts
        result = re.sub(r",\s*,+", ", ", result)
        result = re.sub(r"\s+([,.;!?])", r"\1", result)
        result = re.sub(r",\s*\.", ".", result)
        result = re.sub(r"\.\s*\.", ".", result)
        result = re.sub(r"\s+", " ", result).strip()

        # Ensure initial capitalization
        if result and result[0].islower():
            result = result[0].upper() + result[1:]

        return result

    def humanize(self, text: str) -> str:
        """Full pipeline: Mask technical entities -> Apply anti-AI rules -> Oral cadence -> Restore entities."""
        if not text or not text.strip():
            return ""

        # Stage 1: Mask
        masked, token_map = self.mask_technical_tokens(text)

        # Stage 2: Transform
        cleaned = self.eliminate_ai_patterns(masked)
        cadenced = self.apply_oral_cadence(cleaned)

        # Stage 3: Unmask
        unmasked = self.unmask_technical_tokens(cadenced, token_map)
        return unmasked

    def process_demo_script(
        self, script: DemoScript
    ) -> Tuple[DemoScript, List[Dict[str, Any]]]:
        """Humanizes all scene narrations in a DemoScript and returns diffs."""
        new_script = copy.deepcopy(script)
        diffs: List[Dict[str, Any]] = []

        for scene in new_script.scenes:
            orig = scene.narration
            humanized = self.humanize(orig)
            scene.narration = humanized
            diffs.append({
                "id": scene.id,
                "title": scene.title,
                "original": orig,
                "humanized": humanized,
                "changed": orig.strip() != humanized.strip(),
            })

        return new_script, diffs


def humanize_text(text: str) -> str:
    """Humanizes oral speech for pt-BR, shielding technical tokens and removing AI clichés."""
    humanizer = SpeechHumanizer()
    return humanizer.humanize(text)


def humanize_script(
    script: DemoScript, return_diffs: bool = False
) -> Union[DemoScript, Tuple[DemoScript, List[Dict[str, Any]]]]:
    """Returns a new DemoScript with all scene narrations humanized."""
    humanizer = SpeechHumanizer()
    new_script, diffs = humanizer.process_demo_script(script)
    if return_diffs:
        return new_script, diffs
    return new_script


def format_diff_table(diffs: List[Dict[str, Any]]) -> Any:
    """Renders a rich Table comparing original and humanized narrations."""
    try:
        from rich.table import Table
        from rich.console import Console

        table = Table(title="Revisão de Humanização do Roteiro (pt-BR)")
        table.add_column("Cena / Passo", style="cyan", no_wrap=True)
        table.add_column("Original (com vícios de IA)", style="red")
        table.add_column("Humanizado (Oral)", style="green")

        for d in diffs:
            marker = " *" if d.get("changed") else ""
            table.add_row(f"{d['id']}{marker}", d["original"], d["humanized"])
        return table
    except ImportError:
        # Fallback to plain text representation
        lines = ["=== REVISÃO DE HUMANIZAÇÃO DO ROTEIRO ==="]
        for d in diffs:
            lines.append(f"[{d['id']}]")
            lines.append(f"  ORIGINAL:   {d['original']}")
            lines.append(f"  HUMANIZADO: {d['humanized']}\n")
        return "\n".join(lines)
