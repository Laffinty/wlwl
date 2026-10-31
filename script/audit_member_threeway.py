#!/usr/bin/env python3
# -*- coding: ascii -*-
"""Three-way reconciliation of the public member surface.

  plan sec.6.2  vs  spec repo v1.0.4 candidate text  vs  impl `wlwl namespace`

Usage:
    python script/audit_member_threeway.py [SPEC_REPO] [WLWL_EXE] [PLAN_DOC]

Defaults:
    SPEC_REPO  = D:\\Project\\wlwl-spec-v1
    WLWL_EXE   = <repo>/impl/target/debug/wlwl.exe
    PLAN_DOC   = <repo>/docs/plan/wlwl-v0.12.1-build-plan.md

Notes:
  * Output is deliberately PURE ASCII: the Windows console codepage is GBK and
    any non-ASCII character raises UnicodeEncodeError.
  * No temporary directory is used: the implementation side is read by calling
    `wlwl namespace` directly and parsing the JSON it prints on stdout.
  * Namespace naming differs between the two sides and is mapped here:
      spec `std.http`  ->  impl path `wlwl:std.http`
      spec `ai`        ->  impl path `ai`        (official package)
      spec `agent`     ->  impl path `agent`     (official package)
  * The plan side is read twice:
      - PLAN_BEFORE: a verbatim snapshot of plan sec.6.2 as written BEFORE the
        2026-10-31 documentation fix, so the historical discrepancy stays
        reproducible;
      - PLAN_NOW: parsed live out of the plan document, so the script reports
        the current state of the document instead of trusting the snapshot.

Exit code: 0 when spec <= impl and impl <= spec hold (no missing member, no
unregistered member); 1 otherwise.
"""

import json
import os
import re
import subprocess
import sys

DEFAULT_SPEC_REPO = r"D:\Project\wlwl-spec-v1"
REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_WLWL_EXE = os.path.join(REPO_ROOT, "impl", "target", "debug", "wlwl.exe")
DEFAULT_PLAN_DOC = os.path.join(REPO_ROOT, "docs", "plan", "wlwl-v0.12.1-build-plan.md")

REGISTRY_REL = os.path.join("docs", "stdlib", "wlwl-v1.0-stdlib-registry.md")

# Namespaces introduced by this round: every one of their members is a new
# member, so plan == spec must hold exactly. For the pre-existing namespaces
# the plan table lists only the new members, therefore only "plan claims a
# member the spec does not have" is a defect there.
NEW_NAMESPACES = ("wlwl:std.http", "wlwl:std.log", "wlwl:std.sync", "wlwl:std.signal")

# Global built-ins are NOT namespace members: the plan lists them inside the
# sec.6.2 table (under std.collection) but the spec registers them in
# appendix G, so they are absent from the namespace member table BY DESIGN.
GLOBAL_BUILTINS = ("ARRAY_GET",)

# --- plan sec.6.2 as written BEFORE the 2026-10-31 documentation fix -------
# Verbatim snapshot. Kept so that the three-way gap stays reproducible.
PLAN_BEFORE = {
    "wlwl:std.http": "PARSE_REQUEST BUILD_RESPONSE SERVE_STREAM SERVE ROUTE MATCH_ROUTE "
                     "PARSE_COOKIES SET_COOKIE PARSE_FORM PARSE_MULTIPART STATIC_FILE GET "
                     "POST REQUEST COMPRESS_RESPONSE RATE_LIMITER RULESET_LOAD CLIENT_IP",
    "wlwl:std.log": "LOG LOG_SET_LEVEL LOG_FILE LOG_METRICS",
    "wlwl:std.sync": "COUNTER COUNTER_INC COUNTER_ADD COUNTER_GET SHARED_DICT "
                     "SHARED_DICT_SET SHARED_DICT_GET SHARED_DICT_SNAPSHOT",
    "wlwl:std.signal": "ON_SIGNAL STOP",
    "wlwl:std.net": "TCP_SERVE HTTP_REQUEST_ASYNC TCP_CONNECT",
    "wlwl:std.text": "NFKC CASE_FOLD STRIP_INVISIBLE",
    "wlwl:std.fs": "PATH_CONTAINS PATH_CANONICAL WRITE_FILE_ATOMIC LOCK_FILE UNLOCK_FILE "
                   "FILE_MTIME DIR_ENTRIES FILE_META",
    "wlwl:std.str": "QUERY_PARSE",
    "wlwl:std.format": "FORMAT_NAMED",
    "wlwl:std.test": "ASSERT_MSG ASSERT_TRUE EXPECT_ERR_CODE SKIP TEST_CASES TEMP_DIR "
                     "RUN_TESTS_ASYNC",
    "wlwl:std.regex": "RE_MATCHES_FULL",
}
PLAN_BEFORE = dict((k, set(v.split())) for k, v in PLAN_BEFORE.items())

APPENDIX_A_HEADING_RE = re.compile(r"^##\s+.*\u9644\u5f55\s*A")
HEADING_RE = re.compile(r"^##\s+")
SPEC_ROW_RE = re.compile(r"^\|\s*`([a-z][a-z0-9_.]*)`\s*\|(.*?)\|\s*[^|]*\|")
MEMBER_RE = re.compile(r"`([A-Z][A-Z0-9_]*)`")
PLAN_ROW_RE = re.compile(r"^\|\s*\*{0,2}`std\.([a-z]+)`\*{0,2}[^|]*\|\s*([^|]*)\|")
SECTION_RE = re.compile(r"^###\s+6\.2\b")
NEXT_SECTION_RE = re.compile(r"^#{2,6}\s+")


def fail(msg):
    sys.stderr.write("ERROR: %s\n" % msg)
    sys.exit(2)


def impl_path(spec_ns):
    """Map a spec registry namespace name to the implementation namespace path."""
    if spec_ns.startswith("std."):
        return "wlwl:" + spec_ns
    return spec_ns


def load_impl(exe):
    """Call `wlwl namespace` and parse the JSON it prints on stdout."""
    if not os.path.isfile(exe):
        fail("wlwl executable not found: %s" % exe)
    proc = subprocess.run(
        [exe, "namespace"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        fail("`%s namespace` exited with %d" % (exe, proc.returncode))
    raw = proc.stdout.decode("utf-8-sig", errors="replace")
    try:
        data = json.loads(raw)
    except ValueError as exc:
        fail("cannot parse `wlwl namespace` output as JSON: %s" % exc)
    impl = {}
    for ns in data.get("namespaces", []):
        impl[ns["path"]] = set(ns.get("members", []))
    return impl, data


def load_spec(spec_repo):
    """Read the normative member table (appendix A) of the spec registry."""
    registry = os.path.join(spec_repo, REGISTRY_REL)
    if not os.path.isfile(registry):
        fail("spec registry not found: %s" % registry)
    spec = {}
    in_appendix = False
    with open(registry, encoding="utf-8") as handle:
        for line in handle:
            if APPENDIX_A_HEADING_RE.match(line):
                in_appendix = True
                continue
            if in_appendix and HEADING_RE.match(line):
                break
            if not in_appendix:
                continue
            m = SPEC_ROW_RE.match(line)
            if not m:
                continue
            members = set(MEMBER_RE.findall(m.group(2)))
            if members:
                spec[impl_path(m.group(1))] = members
    if not spec:
        fail("spec registry appendix A not parsed: %s" % registry)
    return spec, registry


def load_plan_now(plan_doc):
    """Parse the live plan sec.6.2 table out of the plan document."""
    if not os.path.isfile(plan_doc):
        return None, "plan document not found: %s" % plan_doc
    plan = {}
    in_section = False
    with open(plan_doc, encoding="utf-8") as handle:
        for line in handle:
            if SECTION_RE.match(line):
                in_section = True
                continue
            if in_section and NEXT_SECTION_RE.match(line):
                break
            if not in_section:
                continue
            m = PLAN_ROW_RE.match(line)
            if not m:
                continue
            members = set(MEMBER_RE.findall(m.group(2)))
            if members:
                plan["wlwl:std." + m.group(1)] = members
    if not plan:
        return None, "plan sec.6.2 table not parsed: %s" % plan_doc
    return plan, plan_doc


def fmt_pairs(pairs):
    return ", ".join("%s:%s" % (ns, mem) for ns, mem in pairs)


def main():
    argv = sys.argv[1:]
    spec_repo = argv[0] if len(argv) > 0 else DEFAULT_SPEC_REPO
    wlwl_exe = argv[1] if len(argv) > 1 else DEFAULT_WLWL_EXE
    plan_doc = argv[2] if len(argv) > 2 else DEFAULT_PLAN_DOC

    impl, raw_data = load_impl(wlwl_exe)
    spec, registry = load_spec(spec_repo)
    plan_now, plan_note = load_plan_now(plan_doc)

    print("=" * 78)
    print("Three-way reconciliation of the public member surface")
    print("  plan sec.6.2 (snapshot before the 2026-10-31 doc fix)")
    print("  spec repo v1.0.4 candidate text : %s" % registry)
    print("  impl                            : %s" % wlwl_exe)
    print("  plan document (live)            : %s" % plan_doc)
    print("=" *78)
    print("%-22s %5s %5s %5s  %s" % ("namespace", "plan", "spec", "impl", "verdict"))
    print("-" * 78)

    plan_only = []
    spec_only = []
    for ns in sorted(PLAN_BEFORE):
        p = PLAN_BEFORE[ns]
        s = spec.get(ns, set())
        i = impl.get(ns, set())
        verdict = []
        if p - s:
            verdict.append("plan-only: " + ",".join(sorted(p - s)))
        if s != i:
            verdict.append("spec!=impl: " + ",".join(sorted(s ^ i)))
        if ns in NEW_NAMESPACES:
            if s - p:
                verdict.append("spec-only(new ns): " + ",".join(sorted(s - p)))
        elif s - p:
            verdict.append("(spec has %d historical member(s) not in the plan table)"
                           % len(s - p))
        if not verdict:
            verdict.append("three-way match")
        print("%-22s %5d %5d %5d  %s" % (ns, len(p), len(s), len(i), "; ".join(verdict)))
        plan_only.extend((ns, m) for m in sorted(p - s))
        if ns in NEW_NAMESPACES:
            spec_only.extend((ns, m) for m in sorted(s - p))

    ns_count = len(raw_data.get("namespaces", []))
    member_count = sum(len(v) for v in impl.values())
    print()
    print("impl totals: namespaces = %d, members = %d" % (ns_count, member_count))
    print("spec totals: namespaces = %d, members = %d"
          % (len(spec), sum(len(v) for v in spec.values())))

    print()
    print("== spec <= impl ? (does the implementation cover every spec member) ==")
    missing = [(ns, m) for ns in sorted(set(spec) | set(impl))
               for m in sorted(spec.get(ns, set()) - impl.get(ns, set()))]
    if missing:
        print("  MISSING in impl: %s" % fmt_pairs(missing))
    else:
        print("  missing in impl: NONE  ->  spec<=impl: no missing")

    print()
    print("== impl <= spec ? (does the implementation expose unregistered members) ==")
    extra = [(ns, m) for ns in sorted(set(spec) | set(impl))
             for m in sorted(impl.get(ns, set()) - spec.get(ns, set()))]
    if extra:
        print("  UNREGISTERED in impl: %s" % fmt_pairs(extra))
    else:
        print("  unregistered in impl: NONE  ->  impl<=spec: every member is registered")

    print()
    print("== plan sec.6.2 (pre-fix snapshot) vs spec: members the plan over-claims ==")
    if plan_only:
        for ns, mem in plan_only:
            print("  plan-only member: %s::%s" % (ns, mem))
        print("  count: %d  ->  %s" % (len(plan_only), fmt_pairs(plan_only)))
    else:
        print("  none")

    print()
    print("== plan sec.6.2 (pre-fix snapshot) vs spec: new-namespace members the plan omits ==")
    if spec_only:
        for ns, mem in spec_only:
            print("  spec-only member: %s::%s" % (ns, mem))
        print("  count: %d  ->  %s" % (len(spec_only), fmt_pairs(spec_only)))
    else:
        print("  none")

    if plan_now is None:
        print()
        print("== live plan document: NOT PARSED (%s) ==" % plan_note)
    else:
        print()
        print("== live plan sec.6.2 vs spec vs impl ==")
        drift = []
        expected = []
        for ns in sorted(set(plan_now) | set(spec) | set(impl)):
            p = plan_now.get(ns, set())
            s = spec.get(ns, set())
            i = impl.get(ns, set())
            p_extra = sorted(p - s)
            for mem in list(p_extra):
                if mem in GLOBAL_BUILTINS:
                    p_extra.remove(mem)
                    expected.append((ns, mem))
            s_extra = sorted(s - p) if ns in NEW_NAMESPACES else []
            s_impl = sorted(s ^ i)
            if p_extra or s_extra or s_impl:
                drift.append((ns, p_extra, s_extra, s_impl))
        for ns, p_only, s_only, s_impl in drift:
            print("  %s: plan-only=%s spec-only=%s spec!=impl=%s"
                  % (ns, p_only, s_only, s_impl))
        if not drift:
            print("  live plan sec.6.2 matches the spec and the implementation")
        for ns, mem in expected:
            print("  %s::%s: global built-in (registered in appendix G, not a namespace "
                  "member) -> expected difference" % (ns, mem))
        print("  live plan sec.6.2 member counts: %s"
              % ", ".join("%s=%d" % (ns, len(plan_now[ns])) for ns in sorted(plan_now)))

    print()
    print("=" * 78)
    ok = not missing and not extra
    print("RESULT: spec<=impl no missing = %s ; impl<=spec no unregistered = %s"
          % (not missing, not extra))
    print("        plan over-claims (pre-fix) = %d member(s)" % len(plan_only))
    print("=" * 78)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
