# Historical update fixtures — retired signing key

These receipts/public signatures describe the original local test. The public
key in `public-key.txt` is **retired**, not the current production or lab key.
No enabled build shipped with it. The private item was retained, not exported.

All old lab app/ZIP/tar copies created for these tests have been removed from the
Mac and disposable guest. The new production key is separate from the lab key.
The staging drivers are now safety-adapted: they require
`KOETT_LAB_UPDATE_SIGNER` and explicitly pass `--lab`; they override the app's key
with `lab-public.txt` and refuse equality with the production public key.
Original executed scripts/manifest hashes remain available at immutable commit
52d7820402db536bdd7496744bf986b510e692a6. Do not treat these adapted drivers as the
exact scripts that produced the historical receipts. No production key may sign
these fixtures again.
