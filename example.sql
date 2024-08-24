select
    str(args.path -> dentry -> d_name.name)
from
    kprobe.vfs_open;

