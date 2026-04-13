    web-search)
        MAX="${PARAM_max:-5}"
        ENCODED=$(printf '%s' "$PARAM_query" | sed 's/ /+/g')
        curl -s "https://html.duckduckgo.com/html/?q=$ENCODED" 2>&1 | \
            grep -oP 'class="result__a"[^>]*href="\K[^"]+' | \
            head -n "$MAX" | while read URL; do
                TITLE=$(curl -s "$URL" 2>/dev/null | grep -oP '<title>\K[^<]+' | head -1)
                echo "- [$TITLE]($URL)"
            done
        ;;
