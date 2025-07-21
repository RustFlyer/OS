# The behavior of fanotify

file events on FILE + FAN_REPORT_FID:

- reports a file handle to FILE.

file events on FILE + FAN_REPORT_DFID_NAME:

- reports a file handle to the parent directory of FILE, along with the name of FILE.

file events on DIR + FAN_REPORT_FID:

- needs FAN_ONDIR.
- reports a file handle to DIR.

file events on DIR + FAN_REPORT_DFID_NAME:

- needs FAN_ONDIR.
- reports a file handle to DIR, along with file name `.`.

directory events on FILE + any:

- no events.

directory events on DIR with dentry FILE modified + FAN_REPORT_FID:

- reports a file handle to DIR.

directory events on DIR with dentry FILE modified + FAN_REPORT_DFID_NAME:

- reports a file handle to DIR, along with the name of FILE.

directory events on DIR with dentry DIRR modified + FAN_REPORT_FID:

- needs FAN_ONDIR.
- reports a file handle to DIR.

directory events on DIR with dentry DIRR modified + FAN_REPORT_DFID_NAME:

- needs FAN_ONDIR.
- reports a file handle to DIR, along with the name of DIRR.
