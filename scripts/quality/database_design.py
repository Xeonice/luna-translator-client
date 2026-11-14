#!/usr/bin/env python3
"""Validate and export the SQLite architecture design, never a product database."""

from __future__ import annotations

import argparse
import csv
import hashlib
import html
import io
import json
from pathlib import Path
import re
import sqlite3
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
BASE = "docs/architecture/database"
SOURCES = {"profile": [f"{BASE}/profile.sql", f"{BASE}/config.sql"], "dictionary": [f"{BASE}/dictionary.sql"]}
OUTPUTS = ("field-dictionary.csv", "table-catalog.csv", "schema-metadata.json", "field-dictionary.md", "relationships.svg")
TABLE_MARKER = re.compile(r"^\s*--\s*@table\s+([A-Za-z_][A-Za-z0-9_]*)\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*(.+?)\s*$", re.M)
CREATE_TABLE = re.compile(r"\bCREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(", re.I)
CJK = re.compile(r"[\u3400-\u9fff]")
FIELD_COLUMNS = ["database", "table", "owner", "requirements", "field", "type", "nullable", "primary_key_position", "default", "foreign_keys", "description", "check_or_declaration", "source", "source_line"]
CATALOG_COLUMNS = ["database", "table", "owner", "requirements", "description", "fields", "primary_key", "foreign_keys", "indexes", "triggers", "strict", "source"]


def _json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _ident(value: str) -> str:
    return '"' + value.replace('"', '""') + '"'


def _comment(line: str) -> tuple[str, str]:
    """Split a trailing SQL -- comment without splitting quoted SQL strings."""
    quote: str | None = None
    i = 0
    while i < len(line):
        char = line[i]
        if quote:
            if char == quote:
                if i + 1 < len(line) and line[i + 1] == quote:
                    i += 2
                    continue
                quote = None
        elif char in "'\"`":
            quote = char
        elif line[i:i + 2] == "--":
            return line[:i].rstrip(), line[i + 2:].strip()
        i += 1
    return line.rstrip(), ""


def _table_bodies(source: str) -> dict[str, tuple[str, int]]:
    """Find CREATE TABLE bodies, respecting strings and SQL comments."""
    result: dict[str, tuple[str, int]] = {}
    for match in CREATE_TABLE.finditer(source):
        start = match.end()
        level, i = 1, start
        quote: str | None = None
        while i < len(source) and level:
            char = source[i]
            if quote:
                if char == quote:
                    if i + 1 < len(source) and source[i + 1] == quote:
                        i += 2
                        continue
                    quote = None
            elif source[i:i + 2] == "--":
                end = source.find("\n", i)
                i = len(source) if end < 0 else end
                continue
            elif source[i:i + 2] == "/*":
                end = source.find("*/", i + 2)
                i = len(source) if end < 0 else end + 2
                continue
            elif char in "'\"`":
                quote = char
            elif char == "(":
                level += 1
            elif char == ")":
                level -= 1
            i += 1
        if level == 0:
            result[match.group(1)] = (source[start:i - 1], source.count("\n", 0, start) + 1)
    return result


def _annotations(root: Path, paths: list[str], errors: list[str]) -> tuple[dict[str, dict[str, Any]], list[dict[str, str]]]:
    tables: dict[str, dict[str, Any]] = {}
    sources = []
    for path in paths:
        try:
            raw = (root / path).read_bytes()
            text = raw.decode("utf-8")
        except (OSError, UnicodeError) as exc:
            errors.append(f"{path}: cannot read UTF-8 DDL: {exc}")
            continue
        if not text.strip():
            errors.append(f"{path}: empty DDL")
        sources.append({"path": path, "sha256": hashlib.sha256(raw).hexdigest(), "sql": text})
        markers = {}
        for match in TABLE_MARKER.finditer(text):
            name, owner, requirements, description = (part.strip() for part in match.groups())
            if name in markers or name in tables:
                errors.append(f"{path}: duplicate @table {name}")
            markers[name] = {"owner": owner, "requirements": [item.strip() for item in requirements.split(",")], "description": description}
        bodies = _table_bodies(text)
        for name, (body, body_line) in bodies.items():
            meta = markers.get(name, {})
            fields = {}
            for offset, line in enumerate(body.splitlines()):
                declaration, description = _comment(line)
                match = re.match(r'^\s*(?:"([^"]+)"|([A-Za-z_][A-Za-z0-9_]*))\s+(TEXT|INTEGER|REAL|BLOB|ANY)\b', declaration, re.I)
                if match:
                    field = match.group(1) or match.group(2)
                    fields[field] = {"description": description, "declaration": declaration.strip().rstrip(","), "source_line": body_line + offset}
            tables[name] = {**meta, "source": path, "field_annotations": fields}
        for name in sorted(set(markers) - set(bodies)):
            errors.append(f"{path}: @table {name} has no CREATE TABLE")
    return tables, sources


def _allowed_references(root: Path, errors: list[str]) -> tuple[set[str], set[str]]:
    try:
        scope = (root / "docs/product/first-product-scope.md").read_text(encoding="utf-8")
        requirements = set(re.findall(r"^\|\s*(FP-\d+)\s*\|", scope, re.M))
        with (root / "docs/architecture/engineering-modules.csv").open(encoding="utf-8", newline="") as handle:
            owners = {row["primary_owner"].strip() for row in csv.DictReader(handle)}
        if not requirements or not owners:
            errors.append("database design: authoritative FP/owner mapping is empty")
        return requirements, owners
    except (OSError, KeyError) as exc:
        errors.append(f"database design: cannot read FP/owner authority: {exc}")
        return set(), set()


def _authorizer(action: int, arg1: str | None, arg2: str | None, *_: Any) -> int:
    # DDL is only inspected in memory. Even a malformed design cannot attach a
    # user DB or VACUUM INTO a real file through this inspection tool.
    if action in (sqlite3.SQLITE_ATTACH, sqlite3.SQLITE_DETACH):
        return sqlite3.SQLITE_DENY
    if action == sqlite3.SQLITE_FUNCTION and (arg2 or "").lower() in {"load_extension", "readfile", "writefile"}:
        return sqlite3.SQLITE_DENY
    return sqlite3.SQLITE_OK


def build_metadata(root: Path) -> tuple[dict[str, Any], list[str]]:
    """Introspect fresh, isolated in-memory databases; never open reader.sqlite3."""
    root = Path(root)
    errors: list[str] = []
    requirements, owners = _allowed_references(root, errors)
    databases = []
    for database, paths in SOURCES.items():
        annotations, sources = _annotations(root, paths, errors)
        connection = sqlite3.connect(":memory:")
        connection.row_factory = sqlite3.Row
        connection.set_authorizer(_authorizer)
        try:
            connection.execute("PRAGMA foreign_keys=ON")
            for source in sources:
                connection.executescript(source["sql"])
            if connection.execute("PRAGMA foreign_keys").fetchone()[0] != 1:
                errors.append(f"{database}: foreign_keys must remain ON")
            integrity = [row[0] for row in connection.execute("PRAGMA integrity_check")]
            if integrity != ["ok"]:
                errors.append(f"{database}: integrity_check: {integrity}")
            foreign_errors = [list(row) for row in connection.execute("PRAGMA foreign_key_check")]
            if foreign_errors:
                errors.append(f"{database}: foreign_key_check: {foreign_errors}")
            table_rows = connection.execute("SELECT name,sql FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").fetchall()
            if not table_rows:
                errors.append(f"{database}: no design tables")
            strict_flags = {row["name"]: bool(row["strict"]) for row in connection.execute("PRAGMA table_list") if row["schema"] == "main"}
            actual_names = {row["name"] for row in table_rows}
            for missing in sorted(set(annotations) - actual_names):
                errors.append(f"{database}: annotated table {missing} not present in SQLite schema")
            tables = []
            for table_row in table_rows:
                name = table_row["name"]
                meta = annotations.get(name, {})
                prefix = f"{database}.{name}"
                if not meta.get("owner") or meta["owner"] not in owners:
                    errors.append(f"{prefix}: missing or unknown owner")
                mapped = meta.get("requirements", [])
                if not mapped or any(item not in requirements for item in mapped) or len(mapped) != len(set(mapped)):
                    errors.append(f"{prefix}: missing, duplicate or unknown FP requirements")
                if not CJK.search(meta.get("description", "")):
                    errors.append(f"{prefix}: missing Chinese table description")
                if not strict_flags.get(name):
                    errors.append(f"{prefix}: must be STRICT")
                foreign_keys: dict[int, dict[str, Any]] = {}
                for row in connection.execute(f"PRAGMA foreign_key_list({_ident(name)})"):
                    fk = foreign_keys.setdefault(row["id"], {"id": row["id"], "columns": [], "references_table": row["table"], "references_columns": [], "on_update": row["on_update"], "on_delete": row["on_delete"], "match": row["match"]})
                    fk["columns"].append(row["from"])
                    fk["references_columns"].append(row["to"])
                fk_list = [foreign_keys[key] for key in sorted(foreign_keys)]
                fields = []
                for row in connection.execute(f"PRAGMA table_xinfo({_ident(name)})"):
                    field_meta = meta.get("field_annotations", {}).get(row["name"], {})
                    if not CJK.search(field_meta.get("description", "")):
                        errors.append(f"{prefix}.{row['name']}: missing trailing Chinese field description")
                    if not field_meta.get("declaration"):
                        errors.append(f"{prefix}.{row['name']}: missing source declaration")
                    fields.append({"name": row["name"], "type": row["type"], "nullable": not bool(row["notnull"] or row["pk"]), "primary_key_position": row["pk"], "default": row["dflt_value"], "hidden": row["hidden"], "foreign_keys": [fk for fk in fk_list if row["name"] in fk["columns"]], **field_meta})
                primary_key = [field["name"] for field in sorted(fields, key=lambda item: item["primary_key_position"]) if field["primary_key_position"]]
                if not primary_key:
                    errors.append(f"{prefix}: primary key required")
                indexes = []
                for row in sorted(connection.execute(f"PRAGMA index_list({_ident(name)})"), key=lambda item: item["name"]):
                    sql_row = connection.execute("SELECT sql FROM sqlite_schema WHERE type='index' AND name=?", (row["name"],)).fetchone()
                    indexes.append({"name": row["name"], "unique": bool(row["unique"]), "origin": row["origin"], "partial": bool(row["partial"]), "columns": [dict(column) for column in connection.execute(f"PRAGMA index_xinfo({_ident(row['name'])})")], "sql": sql_row[0] if sql_row else None})
                triggers = [dict(row) for row in connection.execute("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name", (name,))]
                # FK parent-key mismatches and invalid trigger references may only
                # fail when DML is prepared, even if CREATE TABLE succeeded.
                first_field = fields[0]["name"] if fields else "rowid"
                for statement in (f"INSERT INTO {_ident(name)} DEFAULT VALUES", f"UPDATE {_ident(name)} SET {_ident(first_field)}={_ident(first_field)} WHERE 0", f"DELETE FROM {_ident(name)} WHERE 0"):
                    try:
                        connection.execute("EXPLAIN " + statement).fetchall()
                    except sqlite3.Error as exc:
                        errors.append(f"{prefix}: DML/FK/trigger preparation failed: {exc}")
                tables.append({"name": name, "owner": meta.get("owner", ""), "requirements": mapped, "description": meta.get("description", ""), "source": meta.get("source", ""), "strict": strict_flags.get(name, False), "primary_key": primary_key, "fields": fields, "foreign_keys": fk_list, "indexes": indexes, "triggers": triggers, "create_sql": table_row["sql"]})
            databases.append({"name": database, "purpose": "per_profile_writable" if database == "profile" else "bundled_read_only", "sources": [{"path": item["path"], "sha256": item["sha256"]} for item in sources], "tables": tables})
        except sqlite3.Error as exc:
            errors.append(f"{database}: SQLite design validation failed: {exc}")
        finally:
            connection.close()
    metadata = {"schema_version": 1, "kind": "database_architecture_design", "product_tests_executed": False, "databases": databases}
    metadata["counts"] = metadata_counts(metadata)
    return metadata, errors


def metadata_counts(metadata: dict[str, Any]) -> dict[str, Any]:
    tables = [table for database in metadata["databases"] for table in database["tables"]]
    return {"databases": len(metadata["databases"]), "tables": len(tables), "fields": sum(len(table["fields"]) for table in tables), "foreign_keys": sum(len(table["foreign_keys"]) for table in tables), "indexes": sum(len(table["indexes"]) for table in tables), "triggers": sum(len(table["triggers"]) for table in tables), "tables_by_database": {database["name"]: len(database["tables"]) for database in metadata["databases"]}}


def _csv(columns: list[str], rows: list[dict[str, Any]]) -> str:
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=columns, lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return buffer.getvalue()


def _md_cell(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", "<br>")


def _dictionary_markdown(metadata: dict[str, Any]) -> str:
    counts = metadata["counts"]
    lines = ["# 首版数据库字段字典", "", "由 `scripts/quality/database_design.py --write` 从设计 DDL 与 SQLite PRAGMA 确定性生成。仅为架构设计，不是已安装迁移或产品实测。", "", f"共 {counts['databases']} 类数据库、{counts['tables']} 张表、{counts['fields']} 个字段。`profile` 每配置档独立可写；`dictionary` 为随包只读辞书。SQLite 隐式主键/唯一索引也在目录内。", ""]
    for database in metadata["databases"]:
        lines += [f"## {database['name']}", ""]
        for table in database["tables"]:
            lines += [f"### {table['name']}", "", f"{table['description']}。Owner：{table['owner']}；范围：{', '.join(table['requirements'])}；STRICT：是；来源：`{table['source']}`。", "", "| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |", "| --- | --- | --- | --- | --- | --- |"]
            for field in table["fields"]:
                lines.append("| " + " | ".join(_md_cell(value) for value in (f"`{field['name']}`", field["type"], "是" if field["nullable"] else "否", field["primary_key_position"] or "—", field["default"] if field["default"] is not None else "—", field["description"])) + " |")
            lines += ["", "外键：", ""]
            for fk in table["foreign_keys"]:
                lines.append(f"- `({', '.join(fk['columns'])}) → {fk['references_table']}({', '.join(str(item) for item in fk['references_columns'])})`；ON DELETE {fk['on_delete']}；ON UPDATE {fk['on_update']}。")
            if not table["foreign_keys"]:
                lines.append("无。")
            lines += ["", "索引：", ""]
            for index in table["indexes"]:
                keys = [column["name"] if column["name"] is not None else "表达式/rowid" for column in index["columns"] if column["key"]]
                details = "; ".join(filter(None, ("UNIQUE" if index["unique"] else "", "部分索引" if index["partial"] else "", f"origin={index['origin']}")))
                lines.append(f"- `{index['name']} ({', '.join(keys)})`；{details}。")
                if index["sql"]:
                    lines += ["", "```sql", index["sql"] + ";", "```"]
            if not table["indexes"]:
                lines.append("无独立索引（INTEGER PRIMARY KEY 可由 rowid 实现）。")
            lines += ["", "触发器：" + ("、".join(f"`{trigger['name']}`" for trigger in table["triggers"]) or "无") + "。完整定义见 JSON 与源 DDL。", "", "字段声明与表级 CHECK/约束：", "", "```sql", table["create_sql"] + ";", "```", ""]
    return "\n".join(lines)


def _entity_group(database: str, table: dict[str, Any]) -> str:
    name = table["name"]
    if database == "dictionary":
        return "随包只读辞书"
    if table["source"].endswith("config.sql"):
        return "配置快照与凭据"
    if name in {"games", "game_targets", "sessions", "capture_sources", "source_selections"}:
        return "游戏与取文会话"
    if name in {"raw_events", "raw_event_payloads", "reading_units", "reading_unit_revisions", "source_refs", "lexical_snapshots", "lexical_tokens"}:
        return "原文与本地语言"
    if name in {"purpose_runtime_states", "connection_runtime_states", "request_tasks", "request_attempts", "translations", "corrections", "study_results", "cache_entries"}:
        return "任务授权与阅读结果"
    return "偏好、档身份与工程维护"


def _relationships_svg(metadata: dict[str, Any]) -> str:
    escape = html.escape
    all_tables = {table["name"]: table for database in metadata["databases"] for table in database["tables"]}
    groups: dict[str, list[tuple[str, dict[str, Any]]]] = {}
    for database in metadata["databases"]:
        for table in database["tables"]:
            groups.setdefault(_entity_group(database["name"], table), []).append((database["name"], table))
    group_order = ["游戏与取文会话", "原文与本地语言", "任务授权与阅读结果", "配置快照与凭据", "偏好、档身份与工程维护", "随包只读辞书"]
    height = 450 + sum(66 + ((len(groups.get(name, [])) + 1) // 2) * 122 for name in group_order if groups.get(name))
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 1240 {height}" width="1240" height="{height}">', '<title id="title">首版数据库实体与主要关系</title>', '<desc id="desc">主要外键链概览及全部实体分组目录；完整字段与外键约束见字段字典和schema-metadata.json。仅为设计，不代表产品验收。</desc>', '<defs><marker id="arrow" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7" fill="#8da0b6"/></marker></defs>', '<style>text{font-family:-apple-system,BlinkMacSystemFont,"PingFang SC","Microsoft YaHei",sans-serif} .h{fill:#edf1f7;font-weight:650}.m{fill:#aab7c8;font-size:14px}.key{fill:#b6c6da;font-size:13px}.entity{fill:#1d2939;stroke:#506077;stroke-width:1}.group{fill:#111b2a;stroke:#34445b}.arrow{stroke:#8da0b6;stroke-width:1.5;marker-end:url(#arrow)}</style>', f'<rect width="1240" height="{height}" rx="16" fill="#0b1320"/>', '<text x="28" y="44" class="h" font-size="26">首版数据库 · 实体与主要关系</text>', '<text x="28" y="73" class="m">设计草案；箭头仅概览主要父表→子表外键关系。全部实体在下方，完整约束在字段字典/JSON。</text>']
    paths = [("阅读归属", ["sessions", "reading_units", "reading_unit_revisions", "request_tasks", "translations"]), ("服务配置", ["config_revisions", "connections", "models", "binding_engines", "request_tasks"]), ("离线词典", ["dictionary_sources", "dictionary_entries", "dictionary_forms"])]
    for number, (label, names) in enumerate(paths):
        names = [name for name in names if name in all_tables]
        y = 112 + number * 90
        parts.append(f'<text x="28" y="{y}" class="m">{label}</text>')
        width = min(218, (1180 - 24 * max(0, len(names) - 1)) / max(1, len(names)))
        for index, name in enumerate(names):
            x = 28 + index * (width + 24)
            parts += [f'<rect x="{x}" y="{y+12}" width="{width}" height="40" rx="9" class="entity"/>', f'<text x="{x+10}" y="{y+37}" fill="#e1e9f5" font-size="12.5">{escape(name)}</text>']
            if index:
                previous = names[index - 1]
                linked = any(fk["references_table"] == previous for fk in all_tables[name]["foreign_keys"]) or any(fk["references_table"] == name for fk in all_tables[previous]["foreign_keys"])
                if linked:
                    parts.append(f'<line x1="{x-20}" x2="{x-5}" y1="{y+32}" y2="{y+32}" class="arrow"/>')
    y = 405
    for group in group_order:
        tables = groups.get(group, [])
        if not tables:
            continue
        group_height = 66 + ((len(tables) + 1) // 2) * 122
        parts += [f'<rect x="16" y="{y}" width="1208" height="{group_height-12}" rx="14" class="group"/>', f'<text x="32" y="{y+32}" class="h" font-size="19">{group} · {len(tables)} 表</text>']
        for index, (database, table) in enumerate(tables):
            x, card_y = 32 + (index % 2) * 598, y + 49 + (index // 2) * 122
            description = table["description"]
            if len(description) > 36:
                description = description[:35] + "…"
            parents = sorted({fk["references_table"] for fk in table["foreign_keys"]})
            parent_label = ", ".join(parents[:2]) + (f" +{len(parents)-2}" if len(parents) > 2 else "")
            pk_label = ", ".join(table["primary_key"])
            if len(pk_label) > 56:
                pk_label = pk_label[:53] + "…"
            parts += [f'<rect id="{escape(database+"-"+table["name"])}" x="{x}" y="{card_y}" width="580" height="106" rx="10" class="entity"/>', f'<text x="{x+14}" y="{card_y+23}" class="h" font-size="15">{escape(table["name"])} <tspan class="m">· {table["owner"]} · {len(table["fields"])} 字段</tspan></text>', f'<text x="{x+14}" y="{card_y+47}" class="m">{escape(description)}</text>', f'<text x="{x+14}" y="{card_y+68}" class="key">PK: {escape(pk_label)}</text>', f'<text x="{x+14}" y="{card_y+89}" class="key">FK → {escape(parent_label or "无")}</text>']
        y += group_height
    parts += [f'<text x="28" y="{height-20}" class="m">完整关系、索引、触发器与逐列 CHECK 以确定性导出的 CSV / Markdown / JSON 为准。</text>', "</svg>"]
    return "\n".join(parts) + "\n"


def generated_outputs(root: Path) -> tuple[dict[str, str], dict[str, Any], list[str]]:
    metadata, errors = build_metadata(root)
    if errors:
        return {}, metadata, errors
    fields, catalog = [], []
    for database in metadata["databases"]:
        for table in database["tables"]:
            common = {"database": database["name"], "table": table["name"], "owner": table["owner"], "requirements": ";".join(table["requirements"]), "source": table["source"]}
            catalog.append({**common, "description": table["description"], "fields": len(table["fields"]), "primary_key": ";".join(table["primary_key"]), "foreign_keys": len(table["foreign_keys"]), "indexes": len(table["indexes"]), "triggers": len(table["triggers"]), "strict": "true" if table["strict"] else "false"})
            for field in table["fields"]:
                fields.append({**common, "field": field["name"], "type": field["type"], "nullable": "true" if field["nullable"] else "false", "primary_key_position": field["primary_key_position"], "default": field["default"] if field["default"] is not None else "", "foreign_keys": _json(field["foreign_keys"]), "description": field["description"], "check_or_declaration": field["declaration"], "source_line": field["source_line"]})
    return {f"{BASE}/field-dictionary.csv": _csv(FIELD_COLUMNS, fields), f"{BASE}/table-catalog.csv": _csv(CATALOG_COLUMNS, catalog), f"{BASE}/schema-metadata.json": json.dumps(metadata, ensure_ascii=False, indent=2, sort_keys=True) + "\n", f"{BASE}/field-dictionary.md": _dictionary_markdown(metadata), f"{BASE}/relationships.svg": _relationships_svg(metadata)}, metadata, []


def validate_design(root: Path) -> list[str]:
    """Return design/export errors, without writing or claiming runtime success."""
    outputs, _, errors = generated_outputs(Path(root))
    if errors:
        return errors
    for path, expected in outputs.items():
        try:
            actual = (Path(root) / path).read_text(encoding="utf-8")
        except (OSError, UnicodeError) as exc:
            errors.append(f"{path}: generated output missing/unreadable: {exc}")
            continue
        if actual != expected:
            errors.append(f"{path}: generated output differs; review DDL then run database_design.py --write")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true", help="explicitly regenerate reviewed design exports")
    mode.add_argument("--check", action="store_true", help="validate DDL and compare generated exports (default)")
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    outputs, metadata, errors = generated_outputs(args.root)
    if not errors and args.write:
        for path, content in outputs.items():
            target = args.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content, encoding="utf-8", newline="\n")
    elif not errors:
        errors = validate_design(args.root)
    print(_json({"check": "database-design", "mode": "write" if args.write else "check", "status": "failed" if errors else "passed", "counts": metadata["counts"], "sqlite_version": sqlite3.sqlite_version, "product_tests_executed": False, "errors": errors}))
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
