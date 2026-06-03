" grantklassy vim config file
" vim: set ft=vim:

""""""""""""""""""
" BASIC SETTINGS "
""""""""""""""""""

set number			" Line numbers on
colo elflord			" Color scheme
syntax on			" Syntax highlighting
set hlsearch			" Highlight search
set noet			" No 'expand tab'
set cursorline			" Cursor line
set ic				" Ignore case
set title			" Set the terminal tab title
set modeline			" Use modelines
set nofoldenable		" Don't fold lines of code

" Don't indent my lines automatically
set nocindent
set nosmartindent
set noautoindent
set indentexpr=
filetype indent off
filetype plugin indent off

" Spelling
set spellfile=~/.vim/spell/en.utf-8.add
set spelllang=en
"set spell

""""""""""""
" MAPPINGS "
""""""""""""

" Strip trailing whitespace in current buffer (\w)
" Underlying command: :%s/\s\+$//e
nnoremap <silent> <leader>w :%s/\s\+$//e<CR>

""""""""""""""""
" SET FILETYPE "
""""""""""""""""

" Terraform files
autocmd BufNewFile,BufRead *.hcl set filetype=hcl
autocmd BufNewFile,BufRead *.nomad set filetype=hcl
autocmd BufNewFile,BufRead *.tf set filetype=hcl
autocmd BufNewFile,BufRead Appfile set filetype=hcl

" Yaml files
autocmd BufNewFile,BufRead *.yaml,*.yml,*.tmpl set filetype=yaml
autocmd BufNewFile,BufRead *kubeconfig set filetype=yaml nowrap
autocmd FileType yaml setlocal ts=2 sts=2 sw=2 expandtab

" Markdown files
autocmd BufNewFile,BufRead *.md set filetype=markdown

" Python files
autocmd BufNewFile,BufRead *.py set filetype=python
autocmd FileType python setlocal expandtab tabstop=4 softtabstop=4 shiftwidth=4 autoindent

" cfengine2 files
autocmd BufNewFile,BufRead cf.* set filetype=cfengine

"""""""""""""""""""""""""""
" vim-plug PLUGIN MANAGER "
"""""""""""""""""""""""""""
" call plug#begin('~/.vim/plugged')

" WHITESPACE
" Plug 'https://github.com/ntpeters/vim-better-whitespace'

" call plug#end()

