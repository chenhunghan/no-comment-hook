# Changelog

## [0.4.0](https://github.com/chenhunghan/no-comment-hook/compare/no-comment-hook-v0.3.0...no-comment-hook-v0.4.0) (2026-05-27)


### ⚠ BREAKING CHANGES

* --disable/--enable principle keys are renamed. The group keys (session-doc, general, all) are unchanged.
* NO_COMMENT_HOOK_* environment variables are no longer read. Configure via flags on the hook command in hooks/hooks.json (or in ~/.claude/settings.json for a local-dev install).

### Features

* configure via CLI flags only; drop env-var support ([04c4244](https://github.com/chenhunghan/no-comment-hook/commit/04c42449b3bd3787ca272f29af73d990b25dc07a))
* consolidate principles into 7 non-overlapping categories ([408b5a1](https://github.com/chenhunghan/no-comment-hook/commit/408b5a1f3adcacdc1dac4f24dfa184376020a690))

## [0.3.0](https://github.com/chenhunghan/no-comment-hook/compare/no-comment-hook-v0.2.0...no-comment-hook-v0.3.0) (2026-05-27)


### Features

* cap reviewer thinking for ~6x faster reviews ([c0c2364](https://github.com/chenhunghan/no-comment-hook/commit/c0c236447e281d59283f78f21490047c1f736bf5))


### Performance Improvements

* batch hunks into one reviewer call with a cached system prompt ([93908f7](https://github.com/chenhunghan/no-comment-hook/commit/93908f76cfe9e89c9484ed2b1068fba161ef496c))
* skip agent startup in the reviewer (settings/hooks/MCP/session) ([a507769](https://github.com/chenhunghan/no-comment-hook/commit/a5077697470596d186830a20e6050c84bea65af9))

## [0.2.0](https://github.com/chenhunghan/no-comment-hook/compare/no-comment-hook-v0.1.0...no-comment-hook-v0.2.0) (2026-05-27)


### Features

* initial release ([505de83](https://github.com/chenhunghan/no-comment-hook/commit/505de839cbf434736e25148682ad70c0ba73ae77))

## Changelog
