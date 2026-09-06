# Baseline report

baseline_revision=cf5f84072691e06bbd10208199871fdc3b51828f
binary_path=target/debug/belay
command=belay context compile --focus PLN-20260906T120000-001-context-packet-baseline#t-001 --format agent --budget 256
estimator=src/markdown.rs estimate_tokens: ceil(ASCII UTF-8 bytes / 4) plus non-ASCII Unicode scalar count
required_strings=7
retrieval_procedure_commands=1

The values below are populated from the pinned binary run. `required_present`
records observed presence; it is not a claim that all required information fits
the pre-change budget.

binary_sha256=26b5b212ebb033f8620e04eb1cbf5ebb77e40c87c18902647542741adad4645a
bytes=948
tokens=256
sha256=a995fc8febcee44617e371a9fe963f7314de12c10106d8902bd8b97a6cc0bafa
required_present=4/7
required_results=constraints=present,non_goals=present,assumptions=present,unknowns=absent,acceptance=absent,goal_item=present,evidence=absent
additional_retrieval_commands=0
