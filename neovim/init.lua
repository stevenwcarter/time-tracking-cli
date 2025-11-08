-- ~/.config/nvim/lua/custom/timetracking/init.lua
local M = {}

local preview_buf = nil
local preview_win = nil
local debounce_timer = vim.loop.new_timer()
local config = {
  directory = nil, -- Optional: only track files inside this directory
}

-- Normalize and resolve to absolute path
local function normalize_path(path)
  if not path then return nil end
  -- Expand ~ and remove trailing slash
  local expanded = vim.fn.expand(path)
  if expanded:sub(-1) == "/" then expanded = expanded:sub(1, -2) end
  return expanded
end

local function is_within_directory(filepath)
  if not config.directory then
    return true -- No restriction set
  end
  local dir = normalize_path(config.directory)
  local abs_file = vim.fn.fnamemodify(filepath, ":p")
  return abs_file:find(dir, 1, true) == 1
end

local function update_preview()
  local bufname = vim.api.nvim_buf_get_name(0)
  if not is_within_directory(bufname) then return end

  local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
  local input = table.concat(lines, "\n")

  if not (preview_buf and vim.api.nvim_buf_is_valid(preview_buf)) then
    preview_buf = vim.api.nvim_create_buf(false, true)
  end

  if not (preview_win and vim.api.nvim_win_is_valid(preview_win)) then
    preview_win = vim.api.nvim_open_win(preview_buf, false, {
      relative = "editor",
      width = math.floor(vim.o.columns / 2),
      height = vim.o.lines - 4,
      row = 1,
      col = math.floor(vim.o.columns / 2),
      border = "rounded",
      title = "Time Tracking Preview",
      style = "minimal",
    })
  end

  local handle = vim.fn.jobstart({ "ttcli", "--stdin" }, {
    stdout_buffered = true,
    on_stdout = function(_, data)
      if not data or #data == 0 then return end
      vim.schedule(function() vim.api.nvim_buf_set_lines(preview_buf, 0, -1, false, data) end)
    end,
  })

  vim.fn.chansend(handle, input)
  vim.fn.chanclose(handle, "stdin")
end

function M.show_preview()
  local bufname = vim.api.nvim_buf_get_name(0)
  if not is_within_directory(bufname) then return end

  if debounce_timer and debounce_timer:is_active() then debounce_timer:stop() end
  debounce_timer:start(200, 0, vim.schedule_wrap(update_preview))
end

function M.close_preview()
  if preview_win and vim.api.nvim_win_is_valid(preview_win) then
    vim.api.nvim_win_close(preview_win, true)
    preview_win = nil
  end
end

function M.setup(opts)
  config = vim.tbl_deep_extend("force", config, opts or {})

  vim.api.nvim_create_user_command("TTPreview", M.show_preview, {})
  vim.api.nvim_create_user_command("TTClose", M.close_preview, {})

  vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
    pattern = "*.md",
    callback = function()
      local bufname = vim.api.nvim_buf_get_name(0)
      if is_within_directory(bufname) then M.show_preview() end
    end,
  })

  vim.api.nvim_create_autocmd({ "BufLeave" }, {
    pattern = "*.md",
    callback = function()
      local bufname = vim.api.nvim_buf_get_name(0)
      if is_within_directory(bufname) then M.close_preview() end
    end,
  })
end

return M
