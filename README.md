This crate is 100% empty on non-windows platforms.

## Demo

(in Command Prompt)

```text
D:\git\cad97\win-ftype>cargo run -q -- test.py hi ma
[src\main.rs:11] cmd.with_file_type_association()? = "C:\\Windows\\py.exe" "D:\\git\\cad97\\win-ftype\\test.py" "hi" "ma"
['D:\\git\\cad97\\win-ftype\\test.py', 'hi', 'ma']

D:\git\cad97\win-ftype>test.py hi ma
['D:\\git\\cad97\\win-ftype\\test.py', 'hi', 'ma']
```

## Limitations

Necessarily reimplements the ["command line variables"](https://superuser.com/a/473602)
used in defining file type associations, so behavior may differ from `ShellExecute` there.
The most used placeholders of `%0`/`$1`/`%L` and `%*` should mostly work properly.
However, this implementation only does substitution for the *entire* argument; e.g.
an argument of `/z"%1"` won't get substituted properly. Everything else is a best-effort
implementation of mostly undocumented functionality.

Additionally, this crate *does not* replicate the "functionality" of unquoted variables
being expanded into potentially multiple command line arguments. This is primarily due
to the fact that after doing `CommandLineToArgvW` we lose the information of whether the
variable was quoted or not; we assume the proper quoting was used.
