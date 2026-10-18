# This runs before the first prompt of a real interactive login Bash (--login --noprofile -i).
# Read the normal login startup files exactly once, in Bash's normal order, in this same shell.
# No user file is edited; aliases, functions, shell options and job control stay in this process.
builtin unset PROMPT_COMMAND
[[ -r /etc/profile ]] && builtin source /etc/profile
for __miniterm_login_file in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
    if [[ -r "$__miniterm_login_file" ]]; then
        builtin source "$__miniterm_login_file"
        break
    fi
done
builtin unset __miniterm_login_file

# Keep Linux arguments intact when invoking wsl from MSYS, and resolve a managed
# Windows .cmd Codex bridge. Preserve existing aliases/functions and PATH.
if [[ -n $MSYSTEM && -r $MINITERM_PROMPT_DIR/windows-bridge.bash ]]; then
    if [[ ${MINITERM_AGENT_BASH:-1} != 0 && -n $MINITERM_BASH_EXE ]]; then
        # cygpath uses the active MSYS runtime, so custom Git installations and
        # Unicode/space-containing paths resolve to this session's Bash.
        __miniterm_bash_dir=$(command cygpath -u -- "$MINITERM_BASH_EXE")
        __miniterm_bash_dir=${__miniterm_bash_dir%/*}
        export PATH="$__miniterm_bash_dir:$PATH"
        export CLAUDE_CODE_GIT_BASH_PATH=${CLAUDE_CODE_GIT_BASH_PATH:-$MINITERM_BASH_EXE}
        export CLAUDE_CODE_USE_POWERSHELL_TOOL=${CLAUDE_CODE_USE_POWERSHELL_TOOL:-0}
        builtin unset __miniterm_bash_dir
    fi
    builtin source "$MINITERM_PROMPT_DIR/windows-bridge.bash"
fi

__miniterm_prompt_file=$MINITERM_PROMPT_DIR/state
__miniterm_original_ps1=$PS1
__miniterm_original_promptvars=0
builtin shopt -q promptvars && __miniterm_original_promptvars=1
__miniterm_managed_ps1=
__miniterm_apply_prompt() {
    local __miniterm_status=$?
    # End a program's status row when the real shell is ready for another command.
    builtin printf '\033]133;A\007'
    local -a __miniterm_state
    # mapfile buffers the small file; six separate read calls cause extra MSYS file I/O.
    if builtin mapfile -t __miniterm_state 2>/dev/null < "$__miniterm_prompt_file" && [[ ${#__miniterm_state[@]} == 6 ]]; then
        if [[ ${__miniterm_state[0]} == 1 ]]; then
            builtin shopt -s promptvars
            # Keep literals as variables: expanding their contents never executes $() or `...`.
            __miniterm_prompt_user=${__miniterm_state[2]}
            __miniterm_prompt_host=${__miniterm_state[3]}
            __miniterm_prompt_path=${__miniterm_state[4]}
            __miniterm_prompt_symbol=${__miniterm_state[5]}
            PS1=${__miniterm_state[1]}
            __miniterm_managed_ps1=$PS1
        elif [[ -n $__miniterm_managed_ps1 ]]; then
            [[ $PS1 == "$__miniterm_managed_ps1" ]] && PS1=$__miniterm_original_ps1
            [[ $__miniterm_original_promptvars == 0 ]] && builtin shopt -u promptvars
            __miniterm_managed_ps1=
        fi
    fi
    return "$__miniterm_status"
}
# Bash captured the bootstrap command before sourcing the user's startup files. Run the
# user's newly loaded commands for this first prompt as Bash normally would. Only these
# existing shell commands are evaluated; prompt data and aliases are never evaluated.
for __miniterm_user_prompt_command in "${PROMPT_COMMAND[@]}"; do
    builtin eval -- "$__miniterm_user_prompt_command"
done
builtin unset __miniterm_user_prompt_command
# Array assignment preserves a user's scalar or array PROMPT_COMMAND, with MiniTerm last.
PROMPT_COMMAND+=(__miniterm_apply_prompt)
__miniterm_apply_prompt
