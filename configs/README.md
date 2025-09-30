# Runtime configuration directory

The `system.yaml` file in this directory captures the redundancy topology and
field bus mappings enforced by `configd`. The setup helper copies the template
from `examples/configs/system.yaml` when a configuration is missing, but updated
values should be committed here so that validation succeeds during CI and local
bootstraps.
