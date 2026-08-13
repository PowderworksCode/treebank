if grep -q deps <<< "$CI_JOB_NAME" && { [ "$A" = "$B" ] || [ "$first_cache" ] }; then
  echo yes
fi
