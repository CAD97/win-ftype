#![cfg(windows)]

// References:
// - https://docs.microsoft.com/en-us/windows/win32/shell/fa-intro
// - https://docs.microsoft.com/en-us/windows/win32/shell/fa-associationarray
// - https://docs.microsoft.com/en-us/windows/win32/api/shlwapi/nf-shlwapi-assocquerystringw
// - https://docs.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-commandlinetoargvw
// - https://superuser.com/a/473602
// - https://superuser.com/a/1188400

use normpath::PathExt;
use std::{
    env,
    ffi::OsString,
    io, iter,
    os::windows::ffi::{OsStrExt as _, OsStringExt as _},
    path::Path,
    process::Command,
    result::Result,
};
use windows::{
    core::*,
    Win32::{Foundation::*, System::Memory::*, UI::Shell::*},
};

macro_rules! yeet {
    ($e:expr) => {
        Err($e)?
    };
}

mod sealed {
    pub trait Sealed {}
}
use sealed::Sealed;

pub trait CommandExt: Sized + Sealed {
    fn with_file_type_association(&self) -> io::Result<Self>;
}

impl Sealed for Command {}
impl CommandExt for Command {
    fn with_file_type_association(&self) -> io::Result<Self> {
        let program = {
            let mut program = self.get_program().encode_wide().collect::<Vec<_>>();
            if program.contains(&0) {
                yeet!(io::ErrorKind::InvalidInput);
            }
            program.push(0);
            program
        };

        let extension = find_ext(&program).ok_or(io::ErrorKind::InvalidInput)?;

        let mut len = 0;

        #[allow(nonstandard_style)]
        unsafe {
            let flags =
                // Specifies that when an IQueryAssociations method does not find the
                // requested value under the root key, it should attempt to retrieve
                // the comparable value from the * subkey.
                ASSOCF_INIT_DEFAULTTOSTAR |
                // Specifies that the return string should not be truncated. Instead,
                // return an error value and the required size for the complete string.
                ASSOCF_NOTRUNCATE;
            // A command string associated with a Shell verb.
            let str = ASSOCSTR_COMMAND;
            // A pointer to a null-terminated string that is used to determine the
            // root key; A file name extension, such as .txt.
            let pszAssoc = PCWSTR::from_raw(extension.as_ptr());
            // Set this parameter to NULL if it is not used.
            let pszExtra = PCWSTR::null();
            // Set this parameter to NULL to retrieve the required buffer size.
            let pszOut = PWSTR::null();
            // If pszOut is NULL, the function returns S_FALSE and pcchOut points
            // to the required size, in characters, of the buffer.
            let pcchOut = &mut len;

            AssocQueryStringW(flags as _, str, pszAssoc, pszExtra, pszOut, pcchOut)?;
        }

        let mut command_string = Vec::with_capacity(len as usize + 1);

        #[allow(nonstandard_style)]
        unsafe {
            let flags =
                // Specifies that when an IQueryAssociations method does not find the
                // requested value under the root key, it should attempt to retrieve
                // the comparable value from the * subkey.
                ASSOCF_INIT_DEFAULTTOSTAR |
                // Specifies that the return string should not be truncated. Instead,
                // return an error value and the required size for the complete string.
                ASSOCF_NOTRUNCATE;
            // A command string associated with a Shell verb.
            let str = ASSOCSTR_COMMAND;
            // A pointer to a null-terminated string that is used to determine the
            // root key; A file name extension, such as .txt.
            let pszAssoc = PCWSTR::from_raw(extension.as_ptr());
            // Set this parameter to NULL if it is not used.
            let pszExtra = PCWSTR::null();
            // Pointer to a null-terminated string that, when this function returns
            // successfully, receives the requested string.
            let pszOut = PWSTR::from_raw(command_string.as_mut_ptr());
            // A pointer to a value that, when calling the function, is set to the
            // number of characters in the pszOut buffer. When the function returns
            // successfully, the value is set to the number of characters actually
            // placed in the buffer.
            let pcchOut = &mut len;
            // NOTE: a TOCTOU error is possible here; returning an error is reasonable.
            AssocQueryStringW(flags as _, str, pszAssoc, pszExtra, pszOut, pcchOut)?;
        }

        let command_template: Vec<Vec<u16>>;
        #[allow(nonstandard_style)]
        unsafe {
            let mut argc = 0;
            let lpCmdLine = PCWSTR::from_raw(command_string.as_ptr());
            let pNumArgs = &mut argc;
            let argv = CommandLineToArgvW(lpCmdLine, pNumArgs);
            if argv.is_null() {
                yeet!(windows::core::Error::from(GetLastError()));
            }
            command_template = iter::successors(Some(argv), |p| Some(p.add(1)))
                .take(argc as _)
                .map(|p| (&*p).as_wide().to_owned())
                .collect();
            if LocalFree(argv as _) != 0 {
                yeet!(windows::core::Error::from(GetLastError()));
            }
        };

        let command_0 = with_substitutions(
            command_template.get(0).ok_or(io::ErrorKind::NotFound)?,
            self,
        )?;

        let mut fixed_command = Command::new(&command_0[0]);
        fixed_command.args(&command_0[1..]);
        fixed_command.args(
            command_template[1..]
                .iter()
                .map(|template| with_substitutions(template, self))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten(),
        );
        if let Some(current_dir) = self.get_current_dir() {
            fixed_command.current_dir(current_dir);
        }
        for (key, value) in self.get_envs() {
            if let Some(value) = value {
                fixed_command.env(key, value);
            } else {
                fixed_command.env_remove(key);
            }
        }

        Ok(fixed_command)
    }
}

fn find_ext(wstr: &[u16]) -> Option<&[u16]> {
    let (dot, _) = wstr.iter().enumerate().rfind(|&(_, &c)| c == b'.' as u16)?;
    Some(&wstr[dot..])
}

fn with_substitutions(wstr: &[u16], cmd: &Command) -> io::Result<Vec<OsString>> {
    let eq = |s: &str| s.bytes().map(|b| b as u16).eq(wstr.iter().copied());

    if eq("%0") || eq("%1") || eq("%l") || eq("%L") || eq("%d") || eq("%D") {
        Ok(vec![Path::new(cmd.get_program())
            .normalize()?
            .into_os_string()])
    } else if eq("%~") || eq("%*") {
        Ok(cmd.get_args().map(Into::into).collect())
    } else if eq("%2") {
        #[allow(clippy::iter_nth_zero)]
        Ok(cmd.get_args().nth(0).iter().map(Into::into).collect())
    } else if eq("%3") {
        Ok(cmd.get_args().nth(1).iter().map(Into::into).collect())
    } else if eq("%4") {
        Ok(cmd.get_args().nth(2).iter().map(Into::into).collect())
    } else if eq("%5") {
        Ok(cmd.get_args().nth(3).iter().map(Into::into).collect())
    } else if eq("%6") {
        Ok(cmd.get_args().nth(4).iter().map(Into::into).collect())
    } else if eq("%7") {
        Ok(cmd.get_args().nth(5).iter().map(Into::into).collect())
    } else if eq("%8") {
        Ok(cmd.get_args().nth(6).iter().map(Into::into).collect())
    } else if eq("%9") {
        Ok(cmd.get_args().nth(7).iter().map(Into::into).collect())
    } else if eq("%w") || eq("%W") {
        match cmd.get_current_dir() {
            Some(dir) => Ok(vec![dir.to_owned().into_os_string()]),
            None => Ok(vec![env::current_dir()?.into_os_string()]),
        }
    } else if wstr.len() == 2 && wstr[0] == b'%' as u16 {
        if cfg!(debug_assertions) {
            panic!(
                "unsupported shell substitution %{}",
                char::from_u32(wstr[1] as u32).unwrap_or(char::REPLACEMENT_CHARACTER)
            );
        }
        Err(io::ErrorKind::Unsupported.into())
    } else {
        Ok(vec![OsString::from_wide(wstr)])
    }
}
