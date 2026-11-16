# F06 scholarly factory bridge

`ScholarlySettings::from_resolved` carries F06 Boolean, Integer, Float and Text
scalars unchanged into the three actual scholarly factories. Empty and Python
Unicode-whitespace-only base URLs use each adapter's existing default; nonblank
URLs retain exact bytes. Semantic Scholar's resolved nonempty secret becomes its
header credential. Registration uses caller-owned shared HTTP, admission and
parser lifecycle handles; it does not enable engines or alter selection policy.

The 36 frozen controls in `source-resolved-config.json` were captured through the
original YAML/environment loader, service normalization/discovery and actual
original adapters with deterministic MockTransport. Native tests traverse YAML
and explicit fake environment inputs, F06 resolution, this bridge, registered
factories, complete outcome/request assertions and shutdown. They include blank
base values, raw scalar kinds, environment precedence and credential redaction.
OpenAlex's reviewed native User-Agent identity difference remains explicit.

The temporary producer `/private/tmp/a06-resolved-config-source.py` is manual
A06/G08 evidence only and is frozen in its review packet. Required native gates
consume JSON and never invoke the producer. Retire the exporter at G08 alongside
the original backend. No provider calls, runtime Python fallback or new quota.
This slice does not close issue984 or global/platform resource obligations.
