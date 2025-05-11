use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct CallbackQueryData {
    pub command: CallbackQueryCommand,
    pub args: Vec<i64>,
}

#[derive(Debug, PartialEq)]
pub enum CallbackQueryCommand {
    WRONG,
    IGNORE,
    PRO,
    CON,
}

impl FromStr for CallbackQueryCommand {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "wr" => Ok(CallbackQueryCommand::WRONG),
            "ig" => Ok(CallbackQueryCommand::IGNORE),
            "pro" => Ok(CallbackQueryCommand::PRO),
            "con" => Ok(CallbackQueryCommand::CON),
            _ => Err(anyhow::format_err!("Wrong CallbackQueryCommand")),
        }
    }
}

impl FromStr for CallbackQueryData {
    type Err = anyhow::Error;

    fn from_str(command_str: &str) -> Result<Self, Self::Err> {
        let mut iter = command_str.split_ascii_whitespace();

        let command = iter
            .next()
            .ok_or(anyhow::format_err!("Cannot parse command"))?;
        let command = CallbackQueryCommand::from_str(command)?;
        let mut args = vec![];

        for arg_str in iter {
            let arg = i64::from_str(arg_str)?;
            args.push(arg);
        }
        Ok(CallbackQueryData {
            command,
            args,
        })
    }
}
