"""
Unit tests for the speech humanization engine (R1).
Validates anti-AI pattern elimination, 100% technical entity preservation,
oral cadence breath chunking, and DemoScript processing.
"""

from pathlib import Path
import pytest

from scripts.schema import DemoScript, DemoConfig, Scene, Action
from scripts.humanizer import (
    SpeechHumanizer,
    humanize_text,
    humanize_script,
    format_diff_table,
)


# ---------------------------------------------------------------------------
# 1. Technical Entity Preservation Tests (Invariant Guarantee)
# ---------------------------------------------------------------------------


def test_preserves_css_selectors():
    """Verifies that various CSS selector notations are never altered by humanization."""
    text = (
        "Clique no botão button#btn-salvar e digite o valor no campo "
        "input[name='search'] ou no seletor [data-testid='submit-btn']. "
        "Depois verifique a classe .active-tab."
    )
    result = humanize_text(text)

    assert "button#btn-salvar" in result
    assert "input[name='search']" in result
    assert "[data-testid='submit-btn']" in result
    assert ".active-tab" in result


def test_preserves_xpath_selectors():
    """Verifies that XPath expressions remain 100% intact."""
    text = "O Playwright localiza o elemento //button[@id='submit'] e a linha //div[@class='table']/tr[1]."
    result = humanize_text(text)

    assert "//button[@id='submit']" in result
    assert "//div[@class='table']/tr[1]" in result


def test_preserves_urls_and_endpoints():
    """Verifies that full URLs, relative paths, and endpoints are not modified."""
    text = (
        "Enviamos a requisição para http://localhost:3900/v1/audio/speech "
        "e consultamos a documentação em https://example.com/api/v1/docs. "
        "O redirecionamento ocorre para /dashboard/relatorios."
    )
    result = humanize_text(text)

    assert "http://localhost:3900/v1/audio/speech" in result
    assert "https://example.com/api/v1/docs" in result
    assert "/dashboard/relatorios" in result


def test_preserves_code_and_cli_flags():
    """Verifies that inline code blocks, function calls, and CLI flags are preserved."""
    text = (
        "Execute com a flag --headless e verifique o resultado chamando handleClick(). "
        "O valor retornado deve ser `status === 200`."
    )
    result = humanize_text(text)

    assert "--headless" in result
    assert "handleClick()" in result
    assert "`status === 200`" in result


def test_preserves_technical_acronyms():
    """Verifies that technical terms like JSON, REST, API, UUID, FFmpeg are preserved."""
    text = "A API REST retorna um payload JSON com o UUID da sessão, processado pelo FFmpeg."
    result = humanize_text(text)

    assert "API" in result
    assert "REST" in result
    assert "JSON" in result
    assert "UUID" in result
    assert "FFmpeg" in result


# ---------------------------------------------------------------------------
# 2. Anti-AI Pattern Elimination Tests
# ---------------------------------------------------------------------------


def test_eliminates_not_x_but_y():
    """Verifies elimination of 'não apenas X, mas Y' formulas."""
    before = "Não apenas visualizamos os dados, mas também exportamos relatórios."
    after = humanize_text(before)

    assert "não apenas" not in after.lower()
    assert "mas também" not in after.lower()
    assert "visualizamos os dados" in after.lower()
    assert "exportamos relatórios" in after.lower()


def test_eliminates_dramatic_closers():
    """Verifies removal of clichés like 'Simples assim.', 'No final das contas', etc."""
    before = "A tabela atualiza em tempo real. Simples assim."
    after = humanize_text(before)

    assert "simples assim" not in after.lower()
    assert "tabela atualiza em tempo real" in after.lower()

    before_closer = "O fluxo foi concluído. E é isso."
    after_closer = humanize_text(before_closer)
    assert "e é isso" not in after_closer.lower()


def test_eliminates_staged_runups():
    """Verifies removal of staged announcements like 'Vamos mergulhar em'."""
    before = "Vamos mergulhar em como o formulário de cadastro funciona."
    after = humanize_text(before)

    assert "vamos mergulhar em" not in after.lower()
    assert "vejamos" in after.lower() or "formulário de cadastro funciona" in after.lower()

    before_announcement = "Aqui está o que você precisa saber: o sistema é ágil."
    after_announcement = humanize_text(before_announcement)
    assert "aqui está o que você precisa saber" not in after_announcement.lower()


def test_eliminates_aphorisms_and_deep_talk():
    """Verifies that pseudo-philosophical openings are grounded in reality."""
    before = "No coração do módulo de busca, a filtragem é instantânea."
    after = humanize_text(before)

    assert "no coração do" not in after.lower()
    assert "módulo de busca" in after.lower()

    before_core = "No fim do dia, a velocidade de carregamento é o que conta."
    after_core = humanize_text(before_core)
    assert "no fim do dia" not in after_core.lower()
    assert "na prática" in after_core.lower()


def test_eliminates_forced_triads():
    """Verifies reduction of 3-adjective artificial lists to natural 1-2 descriptors."""
    before = "Este novo fluxo é rápido, prático e intuitivo para toda a equipe."
    after = humanize_text(before)

    # The triad should be collapsed, not contain all 3 words in sequence
    assert "rápido, prático e intuitivo" not in after.lower()
    assert "rápido e prático" in after.lower() or "prático" in after.lower()


def test_replaces_all_dashes_with_commas_or_periods():
    """Verifies that em-dashes, en-dashes, and double hyphens are eliminated."""
    before = "O relatório — gerado na hora — contém todas as métricas -- sem cortes – para auditoria."
    after = humanize_text(before)

    assert "—" not in after
    assert "–" not in after
    assert "--" not in after
    # Dashes replaced with natural punctuation
    assert "O relatório, gerado na hora, contém todas as métricas, sem cortes, para auditoria." in after or "," in after


def test_replaces_ai_cliches():
    """Verifies replacement of overused stock AI words like 'robusto', 'ecossistema', 'crucial'."""
    before = "Criamos um ecossistema robusto onde a validação crucial ocorre em tempo real."
    after = humanize_text(before)

    assert "robusto" not in after.lower()
    assert "ecossistema" not in after.lower()
    assert "crucial" not in after.lower()
    # Meaning preserved with natural terms
    assert "estável" in after.lower() or "sistema" in after.lower()
    assert "importante" in after.lower()


def test_eliminates_chatbot_residue():
    """Verifies complete excision of chatbot conversation wrappers."""
    before = "Com certeza! Aqui vemos o relatório consolidado. Espero que ajude!"
    after = humanize_text(before)

    assert "com certeza" not in after.lower()
    assert "espero que ajude" not in after.lower()
    assert "aqui vemos o relatório consolidado" in after.lower()


# ---------------------------------------------------------------------------
# 3. Oral Cadence and Breath Chunking Tests
# ---------------------------------------------------------------------------


def test_breath_chunking_inserts_pause_in_long_clauses():
    """Verifies that clauses exceeding 16 words receive natural respiratory commas."""
    long_clause = (
        "Quando o usuário clica no botão principal de processamento de dados "
        "o sistema inicia uma rotina que valida todos os registros da tabela "
        "sem travar a interface gráfica do navegador."
    )
    # The original has 28 words without any commas or punctuation
    assert len(long_clause.split()) > 16

    after = humanize_text(long_clause)

    # Must contain an inserted comma
    assert "," in after
    # Must preserve words
    assert "quando o usuário" in after.lower()
    assert "sem travar a interface" in after.lower()


def test_short_clauses_retain_clean_cadence():
    """Verifies that short, well-structured sentences are not over-punctuated."""
    short = "Ao clicar em salvar, a tabela atualiza na hora."
    after = humanize_text(short)

    assert after == "Ao clicar em salvar, a tabela atualiza na hora."


# ---------------------------------------------------------------------------
# 4. DemoScript Processing Tests
# ---------------------------------------------------------------------------


def test_humanize_script_processes_all_scenes_immutably():
    """Verifies that humanize_script creates a new DemoScript and humanizes every scene."""
    script = DemoScript(
        title="Roteiro de Demonstração",
        description="Teste de humanização",
        config=DemoConfig(),
        scenes=[
            Scene(
                id="scene_01",
                title="Abertura",
                narration="Com certeza! Este é um ecossistema robusto. Simples assim.",
                actions=[Action(action="navigate", url="/dashboard")],
            ),
            Scene(
                id="scene_02",
                title="Ação Técnica",
                narration="Clique no botão #btn-filtrar — que é crucial — para ver os dados.",
                actions=[Action(action="click", selector="#btn-filtrar")],
            ),
        ],
    )

    # Process script
    humanized_script = humanize_script(script)

    # Ensure original was not mutated
    assert "Com certeza!" in script.scenes[0].narration
    assert "robusto" in script.scenes[0].narration

    # Verify humanized output
    s1 = humanized_script.scenes[0]
    assert "com certeza" not in s1.narration.lower()
    assert "robusto" not in s1.narration.lower()
    assert "simples assim" not in s1.narration.lower()

    s2 = humanized_script.scenes[1]
    assert "#btn-filtrar" in s2.narration  # Technical token preserved!
    assert "—" not in s2.narration        # Dash removed!
    assert "crucial" not in s2.narration.lower()


def test_humanize_script_with_diffs_and_formatting():
    """Verifies diff tracking and rich formatting output."""
    script = DemoScript(
        title="Demo Diff Test",
        description="Verifica relatório de diff",
        config=DemoConfig(),
        scenes=[
            Scene(
                id="step_01",
                title="Cena 1",
                narration="Um sistema robusto — simples assim.",
            )
        ],
    )

    humanized_script, diffs = humanize_script(script, return_diffs=True)

    assert len(diffs) == 1
    assert diffs[0]["id"] == "step_01"
    assert diffs[0]["changed"] is True
    assert diffs[0]["original"] == "Um sistema robusto — simples assim."
    assert "estável" in diffs[0]["humanized"].lower()

    # Verify table formatting
    formatted = format_diff_table(diffs)
    assert formatted is not None


def test_empty_and_whitespace_handling():
    """Verifies that empty strings or whitespace-only inputs return empty string."""
    assert humanize_text("") == ""
    assert humanize_text("   ") == ""
    assert humanize_text(None) == ""


def test_adjacent_technical_tokens():
    """Verifies that adjacent technical tokens are shielded and restored without corruption."""
    text = "Acesse input[name='search']#btn-salvar em https://api.local/v1?mode=full."
    result = humanize_text(text)
    assert "input[name='search']" in result
    assert "#btn-salvar" in result
    assert "https://api.local/v1?mode=full" in result


def test_integrity_error_raised_if_token_corrupted():
    """Verifies that unmasking raises ValueError if a shield token was left behind."""
    humanizer = SpeechHumanizer()
    with pytest.raises(ValueError, match="Integrity error"):
        humanizer.unmask_technical_tokens("Texto com ___SHIELD_TOKEN_999___ corrompido", {})


# ---------------------------------------------------------------------------
# 5. Adversarial Syntax & Boundary Hardening Regression Tests
# ---------------------------------------------------------------------------


def test_url_and_endpoint_trailing_punctuation_shielding():
    """Verifies that trailing punctuation (.!?;:) on URLs and endpoints is NOT swallowed."""
    text = (
        "Consulte a documentação da nossa aplicação diretamente na página "
        "https://example.com/docs. Depois disso podemos realizar a validação "
        "dos dados com muita calma e cuidado."
    )
    result = humanize_text(text)

    # Must preserve clean URL and not produce trailing comma artifact
    assert "https://example.com/docs." not in result or "https://example.com/docs. Depois" in result
    assert "https://example.com/docs., Depois" not in result
    assert "https://example.com/docs" in result

    # Endpoint at end of sentence
    endpoint_text = "Envie a requisição para /v1/audio/speech. Em seguida acompanhe o log."
    endpoint_result = humanize_text(endpoint_text)
    assert "/v1/audio/speech" in endpoint_result
    assert "/v1/audio/speech." not in endpoint_result or "/v1/audio/speech. Em seguida" in endpoint_result


def test_eliminates_not_x_but_y_preserves_copula():
    """Verifies that 'não é apenas X, mas Y' preserves copular verb 'é' and sentence grammar."""
    t1 = "O painel não é apenas bonito, mas também funcional."
    assert humanize_text(t1) == "O painel é bonito e funcional."

    t2 = "Este sistema não é apenas rápido, mas também seguro."
    assert humanize_text(t2) == "Este sistema é rápido e seguro."

    t3 = "Esse não é apenas um botão, mas um atalho rápido."
    assert humanize_text(t3) == "Esse é um botão e um atalho rápido."

    t4 = "Nós não apenas visualizamos os dados, mas como também exportamos relatórios."
    assert humanize_text(t4) == "Nós visualizamos os dados e exportamos relatórios."


def test_staged_runups_preserve_definite_articles():
    """Verifies that staged run-up substitutions preserve Portuguese definite articles."""
    assert humanize_text("Vamos explorar o painel de controle.") == "Vejamos o painel de controle."
    assert humanize_text("Vamos explorar a lista de clientes.") == "Vejamos a lista de clientes."
    assert humanize_text("Vamos mergulhar no código da aplicação.") == "Vejamos o código da aplicação."
    assert humanize_text("Vamos mergulhar na arquitetura.") == "Vejamos a arquitetura."
    assert humanize_text("Vamos mergulhar em como o formulário funciona.") == "Vejamos como o formulário funciona."


def test_dramatic_closers_anchored_no_false_positives():
    """Verifies that closer removals only trigger at sentence ends and never destroy mid-sentence nouns."""
    # Negative cases (must NOT be stripped!)
    assert humanize_text("Coloque um ponto final na frase anterior.") == "Coloque um ponto final na frase anterior."
    assert humanize_text("Coloque um ponto final.") == "Coloque um ponto final."
    assert humanize_text("Esse é o diferencial competitivo que buscamos.") == "Esse é o diferencial competitivo que buscamos."
    assert humanize_text("Isso muda tudo que tínhamos planejado para o sprint.") == "Isso muda tudo que tínhamos planejado para o sprint."

    # Positive closer removals
    assert humanize_text("A tabela atualiza em tempo real. Simples assim.") == "A tabela atualiza em tempo real."
    assert humanize_text("O resultado é exibido na tela. Ponto final.") == "O resultado é exibido na tela."
    assert humanize_text("Nosso sistema processa em paralelo. Esse é o diferencial.") == "Nosso sistema processa em paralelo."


def test_triad_reduction_preserves_factual_information():
    """Verifies that factual classifications of 3 items are preserved, while rhetorical buzzwords are collapsed."""
    factual = "O decreto abrange nível municipal, estadual e federal."
    assert humanize_text(factual) == "O decreto abrange nível municipal, estadual e federal."

    rhetorical = "Este novo fluxo é rápido, prático e intuitivo para toda a equipe."
    res = humanize_text(rhetorical)
    assert "rápido e prático" in res.lower()
    assert "intuitivo" not in res.lower()


def test_preserves_css_pseudo_classes():
    """Verifies that CSS pseudo-classes are shielded and preserved."""
    text = "Selecione button:disabled e depois li:nth-child(2) na lista."
    res = humanize_text(text)
    assert "button:disabled" in res
    assert "li:nth-child(2)" in res

