#!/usr/bin/env python3
"""Generate/check OpenSpec target specs from the existing authoritative product tables."""
import argparse
import csv
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def render_specs(root):
    source = root / 'docs/product/first-product-scope.md'
    rows = re.findall(r'^\| (FP-\d+) \| ([^|]+) \| ([^|]+) \|$', source.read_text(), re.M)
    if len(rows) != 12 or {row[0] for row in rows} != {f'FP-{number:02}' for number in range(1, 13)}:
        raise ValueError('First-product source table must retain FP-01 through FP-12')
    preamble = lambda name, path: f'# {name}\n\n## Purpose\n\n由 `{path}` 生成的目标规范；实现与产品验收仍未完成。修改源文件后显式重新生成，不把生成成功当产品通过。\n\n## Requirements\n\n'
    body = preamble('首个可用产品', 'docs/product/first-product-scope.md')
    for key, title, expected in rows:
        body += f'### Requirement: {key} {title.strip()}\n客户端 SHALL 满足以下可观察行为：{expected.strip()}\n\n#### Scenario: {key}-visible-outcome\n- **WHEN** 用户执行该功能在当前设计稿中的实际入口旅程\n- **THEN** {expected.strip()}\n\n'
    result = {'first-product': body}
    for capability, filename, key_field, expected_field, requirement_field in [
        ('model-configuration', 'model-configuration-acceptance.csv', 'acceptance_id', 'expected_result', 'related_requirements'),
        ('game-onboarding', 'game-library-window-acceptance.csv', 'case_id', 'expected', 'source_requirements'),
    ]:
        path = root / 'docs/product' / filename
        with path.open(newline='') as stream:
            entries = list(csv.DictReader(stream))
        prefix, count = ('MC', 22) if capability == 'model-configuration' else ('GL', 20)
        if len(entries) != count or {row[key_field] for row in entries} != {f'{prefix}-{number:02}' for number in range(1, count+1)}:
            raise ValueError(f'{capability}: missing or duplicate authoritative acceptance IDs')
        body = preamble(capability, 'docs/product/' + filename)
        for row in entries:
            key = row[key_field]
            body += f'### Requirement: {key} {row["scenario"]}\n客户端 SHALL {row[expected_field]}。\n\n来源需求：`{row[requirement_field]}`。\n\n#### Scenario: {key}-acceptance\n- **WHEN** {row.get("steps") or row["scenario"]}\n- **THEN** {row[expected_field]}\n\n'
        result[capability] = body
    return result


def check(root):
    errors = []
    for capability, expected in render_specs(root).items():
        path = root / 'openspec/specs' / capability / 'spec.md'
        if not path.is_file() or path.read_text() != expected:
            errors.append(f'Product spec drift: {path.relative_to(root)}; update its authoritative PRD first')
    return errors


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true', help='Explicitly regenerate target specs; never marks implementation passed')
    args = parser.parse_args()
    if args.write:
        for capability, text in render_specs(ROOT).items():
            target = ROOT / 'openspec/specs' / capability / 'spec.md'
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text)
    else:
        errors = check(ROOT)
        print('\n'.join(errors) if errors else 'Product source/spec identity: passed (target definitions only)')
        raise SystemExit(bool(errors))
