    wolfram)
        ENCODED=$(printf '%s' "$PARAM_input" | sed 's/ /+/g')
        curl -s "https://api.wolframalpha.com/v1/result?appid=$NEIL_CRED&i=$ENCODED" 2>&1
        ;;
