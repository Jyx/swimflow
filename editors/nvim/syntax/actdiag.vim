" Vim syntax file for the swimflow activity DSL.
"
" No tree-sitter or external dependency: this is plain Vimscript regex syntax.
" It covers the compatibility grammar plus the swimflow extensions documented in
" swimflow (multiple diagrams, ref/href, legends and sequence numbers).
"
" Install by adding editors/nvim to 'runtimepath' (see editors/nvim/README.rst).

if exists("b:current_syntax")
  finish
endif

" Structural keywords.
syntax keyword actdiagKeyword diagram actdiag lane class plugin legend entry
syntax keyword actdiagKeyword numbered numbering

" Attribute names. Unknown attributes are allowed by the language, so the
" generic `name = value` matcher below catches those as well.
syntax keyword actdiagAttribute label color textcolor linecolor
syntax keyword actdiagAttribute width height colwidth colheight
syntax keyword actdiagAttribute shape style number description number_position
syntax keyword actdiagAttribute node_width node_height span_width span_height
syntax keyword actdiagAttribute ref href orientation position title

" Common attribute values. These are ordinary names syntactically, but colouring
" them makes orientation/numbering/legend settings easier to scan.
syntax keyword actdiagConstant off auto topological depth portrait landscape
syntax keyword actdiagConstant none top bottom left right

" Attribute statements of the form `key = value`, including user attributes.
" `\ze` stops before the whitespace and `=`.
syntax match actdiagAttribute "\<[A-Za-z_][A-Za-z0-9_.-]*\ze\s*="

" Numbers: the lexer only accepts leading-dot forms as Number tokens, while
" sequence numbers and pixel sizes are authored as ordinary names. Highlight
" both so `numbered = 2` and `.5` read naturally.
syntax match actdiagNumber "\v-?\.\d+"
syntax match actdiagNumber "\<\d\+\>"

" Operators and delimiters.
syntax match actdiagOperator "<->\|->\|<-\|--"
syntax match actdiagDelimiter "[{}\[\];,=]"

" Comments: // and # to end of line, plus /* ... */ across lines.
syntax region actdiagComment start="//" end="$"
syntax region actdiagComment start="#" end="$"
syntax region actdiagComment start="/\*" end="\*/"

" Strings. Single- and double-quoted strings are line-local; triple-quoted
" strings may span lines. The triple form is defined last so it wins at the
" shared start position.
syntax match actdiagEscape +\\["']+ contained
syntax region actdiagString start=+"+ skip=+\\\\\|\\"+ end=+"+ contains=actdiagEscape oneline
syntax region actdiagString start=+'+ skip=+\\\\\|\\'+ end=+'+ contains=actdiagEscape oneline
syntax region actdiagTripleString start=+\%("""\|'''\)+ end=+\%("""\|'''\)+

" Small files: parse from the beginning so multiline comments/strings and
" block detection stay correct when scrolling.
syntax sync fromstart

highlight def link actdiagKeyword Keyword
highlight def link actdiagAttribute Identifier
highlight def link actdiagConstant Constant
highlight def link actdiagOperator Operator
highlight def link actdiagDelimiter Delimiter
highlight def link actdiagNumber Number
highlight def link actdiagString String
highlight def link actdiagTripleString String
highlight def link actdiagEscape SpecialChar
highlight def link actdiagComment Comment

let b:current_syntax = "actdiag"
