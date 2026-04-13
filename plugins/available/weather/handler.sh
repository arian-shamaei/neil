    weather)
        case "$NEIL_ACTION" in
            current)
                curl -s "https://api.openweathermap.org/data/2.5/weather?q=$PARAM_location&appid=$NEIL_CRED&units=metric" 2>&1
                ;;
            forecast)
                DAYS="${PARAM_days:-3}"
                CNT=$((DAYS * 8))
                curl -s "https://api.openweathermap.org/data/2.5/forecast?q=$PARAM_location&appid=$NEIL_CRED&units=metric&cnt=$CNT" 2>&1
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for weather"
                exit 1
                ;;
        esac
        ;;
