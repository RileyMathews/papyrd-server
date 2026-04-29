registry := "registry.rileymathews.com/rileymathews/papyrd"

deploy version:
    docker build -t {{registry}}:{{version}} .
    docker push {{registry}}:{{version}}
