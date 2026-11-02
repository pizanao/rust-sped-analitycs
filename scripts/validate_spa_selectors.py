#!/usr/bin/env python3
"""Pre-flight selector validator for demo_spa.yaml.

Usage: python3 scripts/validate_spa_selectors.py demo_spa.yaml [--url http://localhost:3000]
Exit 0 when all selectors resolve; exit 1 on any timeout.
"""
from __future__ import annotations
import argparse, sys
from pathlib import Path
import yaml
from playwright.sync_api import sync_playwright, TimeoutError as PWTimeout

TIMEOUT_MS = 8_000

GROUPS: list[tuple[str, list[tuple[str, str, str]]]] = [
    ("/rfb-cnpj", [
        ("abertura",    "text=7.53x",                                  "visible"),
        ("timeline",    "text=Pipeline Stages",                        "visible"),
        ("graficos",    "text=Processing Speed",                       "visible"),
        ("grafo_soc",   'button:has-text("Todos")',                    "visible"),
        ("fts5",        'input[placeholder^="Search by"]',              "visible"),
        ("filtro_fts",  'button:has-text("Clear")',                    "visible"),
        ("makefile",    'button:has-text("Run Selected Target")',       "visible"),
        ("clear_mc",    'button:has-text("Clear")',                     "visible"),
    ]),
    ("/ingestao", [
        ("ingestao",    'input[placeholder^="Buscar por arquivo"]',     "visible"),
        ("upload",      'input[type="file"]',                          "present"),
        ("sse_logs",    'button:has-text("Logs")',                      "visible"),
    ]),
    ("/fiscal", [
        ("fiscal_dre",  'input[placeholder^="Digite o CNPJ"]',         "visible"),
        ("xlsx",        'button:has-text("Exportar")',                   "visible"),
        ("sped_blocks", '[role="tab"]:has-text("J150")',                "visible"),
        ("sped_tab2",   '[role="tab"]:has-text("J100")',                "visible"),
        ("sped_tab3",   '[role="tab"]:has-text("I155")',                "visible"),
        ("sped_tab4",   '[role="tab"]:has-text("0000")',                "visible"),
        ("table_filter",'input[placeholder^="Filtrar nesta tabela"]',   "visible"),
    ]),
    ("/grc", [
        ("grc_nav",     'input[placeholder^="CNPJ ou CPF"]',            "visible"),
        ("expandir",    'button:has-text("Expandir Grafo")',           "visible"),
        ("zoom",        'button[title="Aproximar Zoom"]',               "visible"),
        ("slider",      'input[type="range"]',                          "visible"),
        ("caminho",     'button:has-text("Menor Caminho")',             "visible"),
        ("copilot_btn", 'button:has-text("Copiloto de IA")',           "visible"),
    ]),
    ("/indicadores", [
        ("indicadores", 'button:has-text("Atualizar")',                 "visible"),
        ("select_uf",   "select",                                     "visible"),
    ]),
]

def check(page, sel: str, mode: str) -> bool:
    loc = page.locator(sel).first
    try:
        if mode == "present":
            page.wait_for_timeout(100)
            return loc.count() > 0
        loc.wait_for(state="visible", timeout=TIMEOUT_MS)
        return True
    except PWTimeout:
        return False

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("yaml_path")
    ap.add_argument("--url", default="http://localhost:3000")
    args = (ap.parse_args())
    _ = yaml  # kept for future per-scene loading; page-level check used here

    failed = 0
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 800})

        for route, entries in GROUPS:
            page.goto(f"{args.url}{route}", wait_until="networkidle", timeout=30_000)
            page.wait_for_timeout(1_500)
            for label, sel, mode in entries:
                ok = check(page, sel, mode)
                mark = "✅" if ok else "❌"
                print(f"{mark} {route:16} {label:12}  {sel}")
                if not ok:
                    failed += 1

        browser.close()

    print()
    if failed:
        print(f"{failed} seletor(es) falharam.", file=sys.stderr)
        return 1
    print("Todos os seletores OK.")
    return 0

if __name__ == "__main__":
    sys.exit(main())