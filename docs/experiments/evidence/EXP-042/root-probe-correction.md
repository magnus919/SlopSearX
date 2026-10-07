# Pre-call serialization probe correction

The initial independent field-max probe used short candidate IDs and measured 484,330 bytes. The pinned helper also permits 80-byte candidate IDs, so that probe did not maximize every allowed field. It is retained as the short-ID probe.

The corrected field-max probe includes 80-byte candidate IDs and 64-byte facet IDs and measures 527,520 bytes. Both probes exceed the 384,000-byte bound and require whole-request rejection with zero dispatch. An earlier runner probe measured 525,368 bytes with different identifier choices; that value is not the all-field-max result.

These are local serialization measurements, not endpoint qualification. No provider output was available, and no gate or sampled pool changed.
