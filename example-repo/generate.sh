find packages -type f -name 'opam' | xargs -I {} sh -c 'opam2json {} > {}.json'
