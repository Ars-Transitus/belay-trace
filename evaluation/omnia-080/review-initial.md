# OB-02/03 initial independent review

Verdict: FAIL. Base product 457136c4c1861742d195c8e881823630ad7ebd00.
Reviewed uncommitted helper/test file-set digest:
`aaecfde5e4a57066b01e193c6e9fc8fffd750719f4330504cffb9c2d7c7bec3c`.
Evidence: EVD-50e661e888a89f371727ef6175b74bf4.

- P1: top-level Capsule verified/accepted accepted despite unverified/missing
  criterion and no human acceptance binding.
- P1: preview used AC-001 as Success Criterion without explicit AC -> SC map.
- P2: Python equality allowed schema_version true as const 1.

Eight existing tests passed but adversarial probes reproduced the findings.
A fresh fixer owns only scripts/omnia_contract.py and tests/omnia_contract.
Initial pass tests are not acceptance. Require new independent review after fixes.
