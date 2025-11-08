return {
  dir = vim.fn.stdpath "config" .. "/lua/custom/timetracking",
  name = "timetracking-preview",
  event = "VeryLazy",
  config = function()
    require("custom.timetracking").setup {
      directory = "~/.time-tracking",
    }
  end,
}
