#!/usr/bin/env bash

mkdir -p packages

create_opam_file() {
    local pkg=$1
    local ver=$2
    local content=$3
    local dir="packages/${pkg}/${pkg}.${ver}"
    mkdir -p "$dir"
    echo "$content" > "$dir/opam"
}

# A 1.0.0: Uses a ">" operator in a condition.
create_opam_file "A" "1.0.0" 'opam-version: "2.0"
name: "A"
version: "1.0.0"
depends: [
  ("B" {> "1.0.0"} & ("C"))
]
'

# A 1.1.0: Mixes AND, OR and grouping.
create_opam_file "A" "1.1.0" 'opam-version: "2.0"
name: "A"
version: "1.1.0"
depends: [
  ( ("B" {>= "1.0.0"} & ("E" {!= "1.0.0"}) ) | ("C") ) & ( ("D" {<= "2.1.0"}) | ("E") )
]
'

# A 2.0.0: Uses both ">" and "<" in the same dependency.
create_opam_file "A" "2.0.0" 'opam-version: "2.0"
name: "A"
version: "2.0.0"
depends: [
  "B" {> "1.2.0"} & ( "C" | ( "D" {= "2.0.0" & ! (< "2.5.0")} ) )
]
'

# A 2.1.0: Uses a mix of conjunction and disjunction.
create_opam_file "A" "2.1.0" 'opam-version: "2.0"
name: "A"
version: "2.1.0"
depends: [
  "B" {>= "2.0.0"} & ( "C" {< "2.0.0"} | "E" {>= "1.0.0"} )
]
'

# A 3.0.0: Complex formula with nested groups and negation.
create_opam_file "A" "3.0.0" 'opam-version: "2.0"
name: "A"
version: "3.0.0"
depends: [
  ( "B" {>= "2.0.0"} & "C" {>= "1.5.0"} ) | ( "D" {>= "2.0.0"} & "E" {= "1.0.0"} )
]
'

# B 1.0.0
create_opam_file "B" "1.0.0" 'opam-version: "2.0"
name: "B"
version: "1.0.0"
depends: [
  "E" {= "1.0.0"}
]
'

# B 1.2.0
create_opam_file "B" "1.2.0" 'opam-version: "2.0"
name: "B"
version: "1.2.0"
depends: [
  "C" | "E" {!= "1.1.0"}
]
'

# B 2.0.0
create_opam_file "B" "2.0.0" 'opam-version: "2.0"
name: "B"
version: "2.0.0"
depends: [
  ( "A" {< "3.0.0"} & "E" {>= "1.0.0"} ) | "C"
]
'

# C 1.0.0
create_opam_file "C" "1.0.0" 'opam-version: "2.0"
name: "C"
version: "1.0.0"
depends: []
'

# C 1.5.0
create_opam_file "C" "1.5.0" 'opam-version: "2.0"
name: "C"
version: "1.5.0"
depends: [
  "E" {>= "1.0.0"}
]
'

create_opam_file "D" "2.0.0" 'opam-version: "2.0"
name: "D"
version: "2.0.0"
depends: [
  "E" {>= "2.0.0"} | "C"
]
'

create_opam_file "E" "1.0.0" 'opam-version: "2.0"
name: "E"
version: "1.0.0"
depends: []
'
