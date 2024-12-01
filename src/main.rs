use paste::paste;
use std::env;
use std::process;
use std::str::FromStr;

const RPI_5_VCGENCMD_PMIC_READ_ADC_OUTPUT_ROWS: usize = 12;
const VCGENCMD_EXITED_NON_ZERO_CODE_COOLDOWN: u64 = 5;

macro_rules! vcgencmd_readings {
    ($($type_: tt)*) => {
        paste!{
            #[allow(non_camel_case_types)]
            #[derive(Clone, Copy, Debug)]
            enum AmpereMeasurement<T: Copy + std::ops::Mul<Output = T> + FromStr> {
                $(
                    [<vcgencmd_ $type_>](T),
                )*
            }
            impl<'a, T> TryFrom<(&'a str, &'a str)> for AmpereMeasurement<T>
                where T: Copy + std::ops::Mul<Output = T> + FromStr
            {
                type Error = (&'a str, &'a str);
                fn try_from((str_ampere, str_float): (&'a str, &'a str)) -> Result<Self, Self::Error> {
                    if let (Some(ampere), Some(Ok(float))) = (str_ampere.strip_suffix("_A"), str_float.strip_suffix("A").map(|s| s.parse::<T>())) {
                        match ampere {
                            $(
                                stringify!($type_) => Ok(Self::[<vcgencmd_ $type_>](float)),
                            )*
                            _ => Err((str_ampere, str_float))
                        }
                    } else {
                        Err((str_ampere, str_float))
                    }
                }
            }
        }

        paste!{
            #[allow(non_camel_case_types)]
            #[derive(Clone, Copy, Debug)]
            enum VoltageMeasurement<T: Copy + std::ops::Mul<Output = T> + FromStr> {
                $(
                    [<vcgencmd_ $type_>](T),
                )*
            }
            impl<'a, T> TryFrom<(&'a str, &'a str)> for VoltageMeasurement<T>
                where T: Copy + std::ops::Mul<Output = T> + FromStr
            {
                type Error = (&'a str, &'a str);
                fn try_from((str_voltage, str_float): (&'a str, &'a str)) -> Result<Self, Self::Error> {
                    if let (Some(voltage), Some(Ok(float))) = (str_voltage.strip_suffix("_V"), str_float.strip_suffix("V").map(|s| s.parse::<T>())) {
                        match voltage {
                            $(
                                stringify!($type_) => Ok(Self::[<vcgencmd_ $type_>](float)),
                            )*
                            _ => Err((str_voltage, str_float))
                        }
                    } else {
                        Err((str_voltage, str_float))
                    }
                }
            }
        }

        paste!{
            #[allow(non_camel_case_types)]
            #[derive(Clone, Copy, Debug)]
            enum WattageMeasurement<T: Copy + std::ops::Mul<Output = T> + FromStr> {
                $(
                    [<vcgencmd_ $type_>](T),
                )*
            }
            impl<T> TryFrom<(AmpereMeasurement<T>, VoltageMeasurement<T>)> for WattageMeasurement<T>
                where T: Copy + std::ops::Mul<Output = T> + FromStr
            {
                type Error = (AmpereMeasurement<T>, VoltageMeasurement<T>);
                fn try_from((a, v): (AmpereMeasurement<T>, VoltageMeasurement<T>)) -> Result<Self, Self::Error> {
                    match (a, v) {
                        $(
                            (
                                AmpereMeasurement::[<vcgencmd_ $type_>](a),
                                VoltageMeasurement::[<vcgencmd_ $type_>](v)
                            ) => Ok(Self::[<vcgencmd_ $type_>](a * v)),
                        )*
                        _ => Err((a, v))
                    }
                }
            }
            impl<T> std::ops::Deref for WattageMeasurement<T>
                where T: Copy + std::ops::Mul<Output = T> + std::ops::Add<Output = T> + FromStr
            {
                type Target = T;
                fn deref(&self) -> &Self::Target {
                    match self {
                        $(
                            Self::[<vcgencmd_ $type_>](f) => &f,
                        )*
                    }
                }
            }

            impl<T> From<&WattageMeasurement<T>> for &'static str
                where T: Copy + std::ops::Mul<Output = T> + std::ops::Add<Output = T> + FromStr
            {
                fn from(w: &WattageMeasurement<T>) -> Self {
                    match w {
                        $(
                            WattageMeasurement::<T>::[<vcgencmd_ $type_>](_) => stringify!($type_),
                        )*
                    }
                }
            }
        }
    };
}

vcgencmd_readings! {
    3V7_WL_SW
    3V3_SYS
    1V8_SYS
    DDR_VDD2
    DDR_VDDQ
    1V1_SYS
    0V8_SW
    VDD_CORE
    3V3_DAC
    3V3_ADC
    0V8_AON
    HDMI
}

fn main() {
    let mut vcgencmd = process::Command::new("vcgencmd");
    let thread_sleep_duration: f32 = env::var("SLEEP")
        .map_err(|_| ())
        .and_then(|sleep| sleep.parse::<f32>().map_err(|_| ()))
        .unwrap_or(1.0);

    // assuming RPi `vcgencmd` always produces output in certain fixed relative order
    // s.t. ampere measurement of A comes before that of B
    // iff voltage measurement of A comes before that of B
    let mut amperes = Vec::with_capacity(RPI_5_VCGENCMD_PMIC_READ_ADC_OUTPUT_ROWS);
    let mut voltages = Vec::with_capacity(RPI_5_VCGENCMD_PMIC_READ_ADC_OUTPUT_ROWS);

    while let Ok(ret) = vcgencmd
        .arg("pmic_read_adc")
        .stdout(std::process::Stdio::piped())
        .output()
    {
        if !ret.status.success() {
            println!(
                "\n\n!!!!!!!!! vcgencmd exited with status {:?}\n\n",
                ret.status
            );
            println!(
                "cooling down for {} secs...",
                VCGENCMD_EXITED_NON_ZERO_CODE_COOLDOWN
            );
            std::thread::sleep(std::time::Duration::from_secs(
                VCGENCMD_EXITED_NON_ZERO_CODE_COOLDOWN,
            ));
            continue;
        }

        let vcgencmd = String::from_utf8_lossy(&ret.stdout);

        vcgencmd.lines().for_each(|l| {
            let mut iter = l.trim().split(&[' ', '=']);
            let type_ = iter.next();
            let val = iter.nth(1);

            if let (Some(type_), Some(val)) = (type_, val) {
                if let Ok(v) = VoltageMeasurement::<f32>::try_from((type_, val)) {
                    voltages.push(v)
                } else if let Ok(a) = AmpereMeasurement::<f32>::try_from((type_, val)) {
                    amperes.push(a)
                }
            }
        });

        let (output, sum_of_watts) = amperes
            .drain(..)
            .zip(voltages.drain(..))
            .filter_map(|tuple_of_ampere_and_voltage| {
                WattageMeasurement::try_from(tuple_of_ampere_and_voltage).ok()
            })
            .fold(
                (
                    String::from("====================================\n"),
                    0.0_f32,
                ),
                |(output, sum_of_watts), w| {
                    (
                        output + &format!("Wattage of {}: {}\n", <&'static str>::from(&w), *w),
                        sum_of_watts + *w,
                    )
                },
            );
        let output = output
            + &format!(
                "Total board power: {} watts\n====================================",
                sum_of_watts
            );

        println!("{}", output);

        std::thread::sleep(std::time::Duration::from_secs_f32(thread_sleep_duration));
    }
}
