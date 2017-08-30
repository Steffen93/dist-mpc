use std::io::{Read, Write, self};
use std::thread;
use std::time::Duration;
use std::fs::{self, File};
use std::process::Command;
use protocol::*;

const REMOTEPATH_ALPINE_RELEASE: &'static str = ".alpine-release";
const REMOTEPATH_TEST_BURN: &'static str = "mpc_testburn";

/// Clears the entire terminal screen, moves cursor to top left.
pub fn reset() {
    print!("{}[2J", 27 as char);
    print!("{}[1;1H", 27 as char);
    println!("[MPC] Do not exit this process or shut the system off.");
    println!("");
}

pub fn prompt(s: &str) -> String {
    loop {
        let mut input = String::new();
        reset();
        println!("{}", s);
        println!("\x07");

        if io::stdin().read_line(&mut input).is_ok() {
            println!("Please wait...");
            return (&input[0..input.len()-1]).into();
        }
    }
}

pub struct TemporaryFile {
    path: String,
    f: Option<File>
}

impl TemporaryFile {
    pub fn reset(&mut self) {
        self.f = None;
        self.f = Some(File::open(&self.path).unwrap());
    }
}

impl Read for TemporaryFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.f.as_mut().unwrap().read(buf)
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        // Close the file descriptor...
        self.f = None;

        // Delete the file.
        loop {
            if fs::remove_file(&self.path).is_ok() {
                return;
            }
            println!("Failed to remove file! Trying again...");
            thread::sleep(Duration::from_secs(1));
        }
    }
}

pub fn write_to_dvd(dvd_path: &str, local_path: &str) -> bool {
    println!("Burning...");

    let output = Command::new("/usr/bin/xorriso")
                         .arg("-outdev")
                         .arg("/dev/sr0")
                         .arg("-md5")
                         .arg("on")
                         .arg("-blank")
                         .arg("as_needed")
                         .arg("-map")
                         .arg(local_path)
                         .arg(dvd_path)
                         .arg("-add")
                         .arg(local_path)
                         .arg("--")
                         .arg("-commit")
                         .arg("-close")
                         .arg("on")
                         .output()
                         .expect("failed to execute xorriso");

    //let stderr = String::from_utf8_lossy(&output.stderr);
    //println!("stderr of write: {}", stderr);

    output.status.success()
}

pub enum DvdStatus {
    File(TemporaryFile),
    Blank,
    Error
}

pub fn read_from_dvd(dvd_path: &str, local_path: &str) -> DvdStatus {
    let output = Command::new("/usr/bin/xorriso")
                         .arg("-md5")
                         .arg("on")
                         .arg("-osirrox")
                         .arg("on")
                         .arg("-indev")
                         .arg("/dev/sr0")
                         .arg("-extract")
                         .arg(dvd_path)
                         .arg(local_path)
                         .output()
                         .expect("failed to execute xorriso");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        //println!("stderr of read: {}", stderr);
        if stderr.contains("is blank") {
            return DvdStatus::Blank
        } else {
            // Remove the file in case it did not write fully to local file, but
            // xorriso didn't delete the partial contents. (Not sure if this
            // actually ever happens, but we don't want participants running
            // out of memory.)
            let _ = fs::remove_file(local_path);

            return DvdStatus::Error
        }
    }

    match File::open(local_path) {
        Ok(f) => {
            DvdStatus::File(TemporaryFile {
                path: local_path.into(),
                f: Some(f)
            })
        },
        Err(_) => {
            println!("Error opening file at path {}", local_path);
            DvdStatus::Error
        }
    }
}

pub fn eject() {
    let _ = Command::new("/usr/bin/eject")
            .arg("/dev/sr0")
            .output();
}

pub fn perform_diagnostics() {
    loop {
        if let DvdStatus::File(_) = read_from_dvd(REMOTEPATH_ALPINE_RELEASE, &format!("{}read_from_iso", ::DIRECTORY_PREFIX)) {
            break;
        } else {
            println!("ERROR! There was a problem reading from the drive or you removed the boot disk from the drive.");
            panic!("cannot recover");
        }
    }

    loop {
        if let DvdStatus::File(_) = read_from_dvd(REMOTEPATH_ALPINE_RELEASE, &format!("{}read_from_iso", ::DIRECTORY_PREFIX)) {
            eject();
            prompt("Please remove the disk from the drive and place it somewhere safe.\n\n \
                    Press [ENTER] when ready to continue.");
        } else {
            break;
        }
    }

    eject();
    prompt("Please place a blank DVD into the drive. Press [ENTER] when ready to continue.");

    loop {
        if let DvdStatus::Blank = read_from_dvd(REMOTEPATH_ALPINE_RELEASE, &format!("{}read_from_iso", ::DIRECTORY_PREFIX)) {
            break;
        } else {
            eject();
            prompt("Try again! Please place a blank DVD into the drive. Press [ENTER] when ready to continue.");
        }
    }

    {
        let mut f = File::create(&format!("{}mpc_testburn", ::DIRECTORY_PREFIX)).unwrap();
        f.write(&[0xff, 0xff, 0xfa, 0x00]).unwrap();
        f.flush().unwrap();
    }

    loop {
        if write_to_dvd(REMOTEPATH_TEST_BURN, &format!("{}mpc_testburn", ::DIRECTORY_PREFIX)) {
            break;
        }

        thread::sleep(Duration::from_secs(3));
    }

    if !fs::remove_file(&format!("{}mpc_testburn", ::DIRECTORY_PREFIX)).is_ok() {
        panic!("could not remove local testburn file");
    }

    loop {
        if let DvdStatus::File(mut f) = read_from_dvd(REMOTEPATH_TEST_BURN, &format!("{}mpc_testburn", ::DIRECTORY_PREFIX)) {
            let mut contents = vec![];
            f.read_to_end(&mut contents).unwrap();
            assert!((&*contents) == &[0xff, 0xff, 0xfa, 0x00]);
            break;
        } else {
            println!("ERROR! There was a problem reading or writing with the drive.");
            panic!("cannot recover");
        }
    }

    eject();
    prompt("Please remove the DVD from the drive and label it 'testburn'. It will not be used again.\n\n\
            Press [ENTER] when the drive is clear.");

    loop {
        if let DvdStatus::Error = read_from_dvd(REMOTEPATH_ALPINE_RELEASE, &format!("{}read_from_iso", ::DIRECTORY_PREFIX)) {
            break;
        } else {
            eject();
            prompt("Try again! Please remove the DVD from the drive. Press [ENTER] when the drive is clear.");
        }
    }
}

pub fn hash_of_file<R: Read>(f: &mut R) -> Digest256 {
    Digest256::from_reader(f)
}

pub fn exchange_disc<
    T,
    R1,
    R2,
    F1: Fn(&mut File) -> Result<(), R1>,
    F2: Fn(&mut TemporaryFile, Option<Digest256>) -> Result<T, R2>
>(
    our_disc: &str,
    their_disc: &str,
    our_cb: F1,
    their_cb: F2
) -> T
{
    let newdisc_localpath = &format!("{}disc{}", ::DIRECTORY_PREFIX, our_disc);
    let newdisc_remotepath = &format!("disc{}", our_disc);
    {
        let mut newdisc = File::create(newdisc_localpath).unwrap();
        our_cb(&mut newdisc).ok().unwrap();
    }
    if ::ASK_USER_TO_RECORD_HASHES {
        let mut newdisc = File::open(newdisc_localpath).unwrap();
        let h = hash_of_file(&mut newdisc);

        write_down_disc_please(&h, our_disc);
    }

    let mut already_burned = false;

    loop {
        if already_burned {
            eject();
            prompt(&format!("Insert disc '{}' from the other machine. If the burn of disc '{}' failed,\n\
                             insert another blank disc to burn it again. Press [ENTER] when ready.",
                            their_disc, our_disc));
        } else {
            eject();
            prompt(&format!("Please insert a blank DVD to burn disc '{}'. Then press [ENTER].",
                            our_disc));
        }

        match read_from_dvd(&format!("disc{}", their_disc), &format!("{}disc{}", ::DIRECTORY_PREFIX, their_disc)) {
            DvdStatus::File(mut f) => {
                let h;
                if ::ASK_USER_TO_RECORD_HASHES {
                    h = Some(hash_of_file(&mut f));
                    write_down_disc_please(&h.clone().unwrap(), their_disc);
                    f.reset();
                } else {
                    h = None;
                }

                match their_cb(&mut f, h) {
                    Ok(data) => {
                        let _ = fs::remove_file(newdisc_localpath);

                        return data;
                    },
                    Err(_) => {
                        eject();
                        prompt(&format!("The disc '{}' you inserted may be corrupted. Burn it again \
                                         on the other machine. Then insert the new disc '{}' and \
                                         press [ENTER].", their_disc, their_disc));
                    }
                }
            },
            DvdStatus::Error => {
                eject();
            },
            DvdStatus::Blank => {
                write_to_dvd(newdisc_remotepath, newdisc_localpath);
                eject();
                already_burned = true;

                prompt(&format!("Disc {} has been burned. Label the disc and transfer it to the\n\
                                 other machine. Press [ENTER] when the drive is clear.", our_disc));
            }
        }
    }
}

pub fn write_disc<
    R,
    F: Fn(&mut File) -> Result<(), R>
>(
    our_disc: &str,
    our_cb: F
)
{
    let newdisc_localpath = &format!("{}disc{}", ::DIRECTORY_PREFIX, our_disc);
    let newdisc_remotepath = &format!("disc{}", our_disc);
    {
        let mut newdisc = File::create(newdisc_localpath).unwrap();
        our_cb(&mut newdisc).ok().unwrap();
    }
    if ::ASK_USER_TO_RECORD_HASHES {
        let mut newdisc = File::open(newdisc_localpath).unwrap();
        let h = hash_of_file(&mut newdisc);

        write_down_disc_please(&h, our_disc);
    }

    let mut already_burned = false;
    eject();

    loop {
        if already_burned {
            prompt(&format!("If the burn of disc '{}' failed, you can insert another blank\n\
                             DVD to burn again. Then press [ENTER] to continue.",
                            our_disc));
        } else {
            prompt(&format!("Please insert a blank DVD to burn disc '{}'.\n\n\
                             Then press [ENTER] to continue.",
                            our_disc));
        }

        match read_from_dvd(newdisc_remotepath, newdisc_localpath) {
            DvdStatus::Blank => {
                write_to_dvd(newdisc_remotepath, newdisc_localpath);
                eject();
                already_burned = true;

                prompt(&format!("Disc {} has been burned. Label the disc and transfer it to the\n\
                                 other machine. Press [ENTER] when the drive is clear.", our_disc));
            },
            _ => {
                eject();
            }
        }
    }
}

pub fn read_disc<T, R, F: Fn(&mut TemporaryFile, Option<Digest256>) -> Result<T, R>>(name: &str, message: &str, cb: F) -> T {
    prompt(message);

    loop {
        match read_from_dvd(&format!("disc{}", name), &format!("{}disc{}", ::DIRECTORY_PREFIX, name)) {
            DvdStatus::File(mut f) => {
                let h;
                if ::ASK_USER_TO_RECORD_HASHES {
                    h = Some(hash_of_file(&mut f));
                    write_down_disc_please(&h.clone().unwrap(), name);
                    f.reset();
                } else {
                    h = None;
                }

                match cb(&mut f, h) {
                    Ok(data) => {
                        return data;
                    },
                    Err(_) => {
                        eject();
                        prompt(&format!("The disc you inserted may be corrupted. Burn it again \
                                on the other machine.\n\n{}", message));
                    }
                }
            },
            DvdStatus::Error => {
                eject();
                prompt(message);
            },
            DvdStatus::Blank => {
                eject();
                prompt(&format!("You placed a blank DVD in the drive, but we're expecting \
                                 disc '{}'.\n\n{}", name, message));
            }
        }
    }
}

pub fn write_down_disc_please(h: &Digest256, name: &str) {
    loop {
        if "recorded" == prompt(&format!("Please write down and publish the string: {}\n\
                                          It is the hash of disc '{}'.\n\n\
                                          Type 'recorded' and press [ENTER] to confirm you've written it down.",
                                          h.to_string(),
                                          name)) {
            break;
        }
    }
}
