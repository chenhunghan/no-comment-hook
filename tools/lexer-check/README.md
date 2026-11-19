# lexer-check

Scores the plugin's comment lexer (`src/lexer.rs`) against tree-sitter on real
source files. Run it after changing the lexer to check nothing regressed, and to
find constructs worth handling. It lives outside the plugin's build, so the
plugin itself keeps zero dependencies.

```sh
cd tools/lexer-check
find ~/src/some-repo -name '*.rb' | cargo run --release
```

Output per run:

```
Ruby     files= 883 (skipped   0 with parse errors)  comments: tree-sitter= 56310 lexer= 56275  recall= 99.93%  precision=100.00%
   MISS  path/to/file.rb:81: # a comment the lexer did not find
   EXTRA path/to/file.rb:12: text the lexer took for a comment
```

Feed one language per run; the language comes from each file's extension. For
Python it also scores docstrings. A `MISS` usually means a string or literal the
lexer misread just before that line, which then hid the comments after it.
