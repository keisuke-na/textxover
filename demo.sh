#!/bin/bash
# Demo script: simulates live audience reactions

BASE="http://localhost:8080"

comments=(
  "Great talk!"
  "So cool!"
  "Amazing!"
  "Love this"
  "Wow"
  "Nice work!"
  "Awesome presentation"
  "This is incredible"
  "Well done!"
  "Learned a lot"
  "Super helpful"
  "Thanks!"
  "Mind blown"
  "Brilliant"
  "Keep it up!"
)

emojis=("👍" "❤️" "😂" "🎉" "👏" "😮" "🔥" "😭")
colors=("#FFFFFF" "#FF6B6B" "#4ECDC4" "#FFE66D" "#A8E6CF" "#FF8B94" "#95E1D3")
sizes=("small" "medium" "big")

send_comment() {
  local text="${comments[$((RANDOM % ${#comments[@]}))]}"
  local color="${colors[$((RANDOM % ${#colors[@]}))]}"
  local size="${sizes[$((RANDOM % ${#sizes[@]}))]}"
  curl -s -X POST "$BASE/comment" \
    -H "Content-Type: application/json" \
    -d "{\"text\":\"$text\",\"color\":\"$color\",\"size\":\"$size\"}" > /dev/null &
}

send_emoji() {
  local emoji="${emojis[$((RANDOM % ${#emojis[@]}))]}"
  curl -s -X POST "$BASE/comment" \
    -H "Content-Type: application/json" \
    -d "{\"text\":\"$emoji\",\"size\":\"big\"}" > /dev/null &
}

send_firework() {
  local x=$(python3 -c "import random; print(round(0.1 + random.random() * 0.8, 2))")
  local y=$(python3 -c "import random; print(round(0.1 + random.random() * 0.8, 2))")
  curl -s -X POST "$BASE/effect" \
    -H "Content-Type: application/json" \
    -d "{\"type\":\"firework\",\"x\":$x,\"y\":$y}" > /dev/null &
}

echo "Demo started. Press Ctrl+C to stop."

while true; do
  # Burst: send many things at once
  burst=$((4 + RANDOM % 6))
  for ((i=0; i<burst; i++)); do
    roll=$((RANDOM % 10))
    if [ $roll -lt 2 ]; then
      send_emoji
    elif [ $roll -lt 4 ]; then
      send_comment
    else
      send_firework
    fi
  done
  sleep $(python3 -c "import random; print(round(0.05 + random.random() * 0.3, 2))")
done
