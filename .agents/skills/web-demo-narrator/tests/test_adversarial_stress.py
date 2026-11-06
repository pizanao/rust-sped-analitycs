"""
Adversarial Stress Test Suite for Milestone 1: Speech Humanizer (R1).
Empirically tests extreme, hostile, and boundary inputs against humanize_text and humanize_script:
1. Nested CSS selectors with child/sibling combinators and pseudo-classes
2. Complex URLs with queries, fragments, ports, and punctuation boundaries
3. Ambiguous punctuation, quotes, brackets, and dashes inside/outside code tokens
4. Heavy AI patterns interspersed with technical entities
5. Scale and boundary conditions (100+ tokens, multiline, emojis, script immutability)
"""

import sys
from pathlib import Path

# Ensure package root is in sys.path for standalone script execution
PACKAGE_ROOT = Path(__file__).resolve().parent.parent
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import pytest

from scripts.schema import DemoScript, DemoConfig, Scene, Action
from scripts.humanizer import SpeechHumanizer, humanize_text, humanize_script


def test_adversarial_nested_css_selectors():
    """Validates that deeply nested CSS selectors with combinators and pseudo-classes are 100% preserved."""
    selectors = [
        "div.container > ul:nth-child(2) > li[data-active='true']",
        "main > section.content:first-of-type > div[role='alert'] button.close",
        "nav.navbar-nav > li.nav-item.dropdown > a#navbarDropdown[aria-expanded='false']",
        "form#login-form input[type='email']:focus + span.error-msg",
        "table.table-striped > tbody > tr:nth-of-type(odd) td:last-child > a[href*='/edit/']",
        "div[data-testid=\"item-row\"]:not([aria-hidden=\"true\"]) > span.badge--success",
        "button:has(svg.icon--check)",
        "ul.list > li:nth-last-child(3n+1)",
        "button[data-action='elevar para o próximo nível']",
        "div.container > span[data-role='ecossistema-robusto']",
    ]

    for sel in selectors:
        text = f"Clique com atenção no elemento {sel} para abrir a visualização detalhada."
        result = humanize_text(text)
        assert sel in result, f"Corrupted or missing CSS selector: {sel} in output: {result}"


def test_adversarial_complex_urls():
    """Validates that complex URLs with queries, fragments, ports, and boundaries are 100% preserved."""
    url_cases = [
        "https://example.com/api/v1/resource?id=123&filter=all#section",
        "https://sub.domain.co.uk:8080/v2/items?query=search+term&limit=50&offset=0#top-results",
        "http://127.0.0.1:3900/v1/health?verbose=1&check=all",
        "https://user:pass@example.com:8443/api/v2/stream?token=abc-123#live",
        "https://example.com/oauth/authorize?response_type=code&client_id=xyz&redirect_uri=https%3A%2F%2Fapp.com%2Fcallback&scope=read%20write#auth",
    ]

    for url in url_cases:
        # Test in diverse contextual wrappers
        contexts = [
            f"Acesse {url} para prosseguir.",
            f"Consulte ({url}) no navegador.",
            f'Abra "{url}" imediatamente.',
            f"Visite '{url}' para autenticar.",
            f"Acesse {url}, e depois continue.",
            f"Verifique a URL {url}.",
            f"Já conhece {url}?",
            f"Veja em {url}!",
        ]
        for ctx in contexts:
            result = humanize_text(ctx)
            assert url in result, f"URL {url} corrupted in context: {ctx} -> {result}"


def test_adversarial_ambiguous_punctuation_and_dashes():
    """Validates proper handling of dashes, quotes, and punctuation inside and outside code tokens."""
    # 1. Dashes outside code must be converted to commas; dashes inside code must be preserved
    text_dashes = (
        'O comando --headless — com certeza — executa `curl -X POST --header '
        '"Content-Type: application/json" https://api.local/v1/audio/speech` -- sem falhas.'
    )
    res_dashes = humanize_text(text_dashes)
    assert "--headless" in res_dashes, "CLI flag --headless was corrupted"
    assert '`curl -X POST --header "Content-Type: application/json" https://api.local/v1/audio/speech`' in res_dashes, "Code block corrupted"
    assert "—" not in res_dashes, "Em-dash was not converted"
    assert "-- sem falhas" not in res_dashes, "Double hyphen was not converted"
    assert "com certeza" not in res_dashes.lower(), "Chatbot cliché remained"

    # 2. BEM class selectors with hyphens preserved
    text_bem = "O container div.block__element--modifier deve exibir a classe .btn--primary-active."
    res_bem = humanize_text(text_bem)
    assert "div.block__element--modifier" in res_bem
    assert ".btn--primary-active" in res_bem

    # 3. XPath with quotes and attributes
    text_xpath = "Localize //div[@id='content-body']//button[contains(@class, 'btn--action') and @data-ref='step-1'] na tela."
    res_xpath = humanize_text(text_xpath)
    assert "//div[@id='content-body']//button[contains(@class, 'btn--action') and @data-ref='step-1']" in res_xpath

    # 4. Code block containing internal AI clichés must NOT be humanized
    text_code_ai = "Veja a função `// Um ecossistema robusto — simples assim` no script."
    res_code_ai = humanize_text(text_code_ai)
    assert "`// Um ecossistema robusto — simples assim`" in res_code_ai, "Code token content was incorrectly modified"


def test_adversarial_multiple_ai_patterns_with_code():
    """Validates simultaneous elimination of multiple AI patterns with intermixed code tokens."""
    complex_text = (
        "Vamos mergulhar em como não apenas configuramos a URL "
        "https://example.com/api/v1/resource?id=123&filter=all#section, "
        "mas também interagimos com o elemento "
        "div.container > ul:nth-child(2) > li[data-active='true'] "
        "— que é um ecossistema robusto — chamando login() com a flag --headless. "
        "No coração do módulo, o processo é rápido, prático e intuitivo. "
        "No final das contas, o seletor //button[@id='confirm-action']/span[text()='OK'] "
        "recebe o evento handleClick(). Simples assim. E é isso."
    )

    out = humanize_text(complex_text)

    # 1. Technical tokens must be 100% intact
    expected_tokens = [
        "https://example.com/api/v1/resource?id=123&filter=all#section",
        "div.container > ul:nth-child(2) > li[data-active='true']",
        "login()",
        "--headless",
        "//button[@id='confirm-action']/span[text()='OK']",
        "handleClick()",
    ]
    for token in expected_tokens:
        assert token in out, f"Technical token missing/corrupted: {token}"

    # 2. AI patterns must be eradicated
    forbidden_patterns = [
        "vamos mergulhar em",
        "não apenas",
        "mas também",
        "—",
        "ecossistema",
        "robusto",
        "no coração do",
        "rápido, prático e intuitivo",
        "no final das contas",
        "simples assim",
        "e é isso",
    ]
    for pat in forbidden_patterns:
        assert pat.lower() not in out.lower(), f"Forbidden AI pattern remained in text: '{pat}'"


def test_adversarial_scale_and_boundaries():
    """Stress tests high token count, emojis, multiline strings, and script immutability."""
    # 1. 100 technical tokens
    tokens_100 = [f"#btn-id-{i}" for i in range(100)]
    text_100 = "Clique em " + " e depois em ".join(tokens_100) + "."
    out_100 = humanize_text(text_100)
    for t in tokens_100:
        assert t in out_100, f"Token {t} lost in scale test"

    # 2. Emojis and unicode
    text_emoji = "👉 Acesse o botão button#btn-salvar e vá para https://app.io/v1 🚀!"
    out_emoji = humanize_text(text_emoji)
    assert "button#btn-salvar" in out_emoji
    assert "https://app.io/v1" in out_emoji
    assert "👉" in out_emoji
    assert "🚀" in out_emoji

    # 3. Multiline with blank lines
    text_multiline = "Linha 1 com #btn-1.\n\nLinha 2 com https://site.com/v1?a=1#frag.\nLinha 3 com `test()`."
    out_multiline = humanize_text(text_multiline)
    assert "#btn-1" in out_multiline
    assert "https://site.com/v1?a=1#frag" in out_multiline
    assert "`test()`" in out_multiline

    # 4. Long clause (>35 words) breath chunking
    long_clause = (
        "Quando o operador do sistema VoiceStudio inicia o processo de gravação automatizada "
        "através da interface web do navegador Playwright o script carrega todas as cenas configuradas "
        "no arquivo YAML e executa os passos necessários para gerar o vídeo final em alta definição."
    )
    out_long = humanize_text(long_clause)
    assert "," in out_long, "Breathing comma was not inserted into long clause"

    # 5. Full DemoScript immutability under adversarial scenes
    script = DemoScript(
        title="Stress Test Script",
        description="Testing immutability",
        config=DemoConfig(),
        scenes=[
            Scene(
                id="s1",
                title="Scene 1",
                narration="Um ecossistema robusto — simples assim.",
                actions=[Action(action="click", selector="button#btn-save")]
            )
        ]
    )
    new_script, diffs = humanize_script(script, return_diffs=True)
    assert "robusto" in script.scenes[0].narration, "Original script was mutated!"
    assert "robusto" not in new_script.scenes[0].narration.lower(), "AI word remained in new script"
    assert len(diffs) == 1 and diffs[0]["changed"] is True


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
