" Detect swimflow activity diagram files.
"
" `setfiletype` only changes the filetype when it is still empty, so another
" plugin or a modeline keeps precedence.

autocmd BufNewFile,BufRead *.diag,*.actdiag setfiletype actdiag
