#!/usr/bin/env -S just --justfile

# --| Cross platform shebang ----------
# --|----------------------------------
shebang := if os() == 'windows' {
  'pwsh.exe'
} else {
  '/usr/bin/env -S pwsh -noprofile -nologo'
}

set shell := ["/usr/bin/env", "pwsh" ,"-noprofile", "-nologo", "-c"]
set windows-shell := ["pwsh.exe","-NoLogo", "-noprofile", "-c"]

build := './scripts/build.ps1'

# --| Actions -------------------------
# --|----------------------------------

# --| Test ------------------
# --|------------------------

test run debug='no': 
  just _test-{{os()}} {{run}} {{debug}}

_test-linux run debug:
  #!{{shebang}}
  cargo test

_test-windows run debug:
  # Do Windows Things

# --| Build -----------------
# --|------------------------

build source='term': 
  just _build-{{os()}} {{source}}

_build-linux source:
  #!{{shebang}}
  . {{build}}
  RunBuild {{source}}
 
_build-windows source:
  # Do Windows Things

#!{{shebang}}
# . {{build_steps}}
# RunBuild {{run}}
