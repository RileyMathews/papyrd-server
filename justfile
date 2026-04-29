release tag:
    git tag "{{tag}}" HEAD
    git push origin "refs/tags/{{tag}}"
