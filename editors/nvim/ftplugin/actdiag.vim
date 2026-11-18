" Filetype settings for swimflow activity diagrams.

if exists("b:did_ftplugin")
  finish
endif
let b:did_ftplugin = 1

setlocal commentstring=//%s
setlocal comments=://
setlocal suffixesadd=.diag,.actdiag

" Names may contain '-' and '.', so let word motions stay inside them.
setlocal iskeyword+=.,-

" Optional: enable syntax folding for diagram/lane blocks with
"   setlocal foldmethod=syntax
" The syntax file intentionally does not force folding on.

let b:undo_ftplugin = "setlocal commentstring< comments< suffixesadd< iskeyword<"
